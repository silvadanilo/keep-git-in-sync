use chrono::prelude::*;
use git2::{Commit, Direction, IndexAddOption, ObjectType, Oid, Repository, StatusOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ffi::OsStr;

pub fn repo(folder: &str) -> Option<Repository> {
    match Repository::open(folder) {
        Ok(repo) => Some(repo),
        Err(_) => None,
    }
}

pub fn submit(repo: &Repository) -> Result<(), git2::Error> {
    println!("Submitting...");
    if is_modified(repo) {
        pull(repo)?;
        add_all_and_commit(
            repo,
            format!(
                "Auto-Commit at: {}",
                Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
            ),
        )?;
        push(repo, "")?;
        Ok(())
    } else {
        Ok(())
    }
}

fn is_modified(repo: &Repository) -> bool {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut opts));

    !statuses.unwrap().is_empty()
}


fn add_all_and_commit(repo: &Repository, message: String) -> Result<Oid, git2::Error> {
    let mut index = repo.index()?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .expect("Cannot add file to git index");
    index.write().expect("Cannot write git index");

    let oid = index.write_tree()?;
    let signature = repo.signature().expect("Cannot fetch default signature");
    let parent_commit = find_last_commit(&repo)?;
    let tree = repo.find_tree(oid)?;
    repo.commit(
        Some("HEAD"),      //  point HEAD to our new commit
        &signature,        // author
        &signature,        // committer
        &message[..],      // commit message
        &tree,             // tree
        &[&parent_commit], // parents
    )
}

fn pull(repo: &Repository) -> Result<(), git2::Error> {
    execute_git_fn(
        repo.workdir().unwrap(),
        &["pull", "--rebase", "origin", "master"]
    );

    Ok(())


    // let remote_name = "origin";
    // let remote_branch = "master";
    // let mut remote = repo.find_remote(remote_name)?;
    // let fetch_commit = do_fetch(repo, &[remote_branch], &mut remote)?;
    // do_merge(&repo, &remote_branch, fetch_commit)
}

fn find_last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
    let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
    obj.into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find commit"))
}

fn do_fetch<'a>(
    repo: &'a git2::Repository,
    refs: &[&str],
    remote: &'a mut git2::Remote,
) -> Result<git2::AnnotatedCommit<'a>, git2::Error> {
    let mut cb = git2::RemoteCallbacks::new();

    // Print out our transfer progress.
    cb.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            print!(
                "Resolving deltas {}/{}\r",
                stats.indexed_deltas(),
                stats.total_deltas()
            );
        } else if stats.total_objects() > 0 {
            print!(
                "Received {}/{} objects ({}) in {} bytes\r",
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            );
        }
        io::stdout().flush().unwrap();
        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(cb);
    // Always fetch all tags.
    // Perform a download and also update tips
    fo.download_tags(git2::AutotagOption::All);
    println!("Fetching {} for repo", remote.name().unwrap());
    remote.fetch(refs, Some(&mut fo), None)?;

    // If there are local objects (we got a thin pack), then tell the user
    // how many objects we saved from having to cross the network.
    let stats = remote.stats();
    if stats.local_objects() > 0 {
        println!(
            "\rReceived {}/{} objects in {} bytes (used {} local \
             objects)",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes(),
            stats.local_objects()
        );
    } else {
        println!(
            "\rReceived {}/{} objects in {} bytes",
            stats.indexed_objects(),
            stats.total_objects(),
            stats.received_bytes()
        );
    }

    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    Ok(repo.reference_to_annotated_commit(&fetch_head)?)
}

fn do_merge<'a>(
    repo: &'a Repository,
    remote_branch: &str,
    fetch_commit: git2::AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    // 1. do a merge analysis
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    // 2. Do the appopriate merge
    if analysis.0.is_fast_forward() {
        let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
        normal_merge(&repo, &head_commit, &fetch_commit)?;
        // println!("Doing a fast forward");
        // // do a fast forward
        // let refname = format!("refs/heads/{}", remote_branch);
        // match repo.find_reference(&refname) {
        //     Ok(mut r) => {
        //         fast_forward(repo, &mut r, &fetch_commit)?;
        //     }
        //     Err(_) => {
        //         // The branch doesn't exist so just set the reference to the
        //         // commit directly. Usually this is because you are pulling
        //         // into an empty repository.
        //         repo.reference(
        //             &refname,
        //             fetch_commit.id(),
        //             true,
        //             &format!("Setting {} to {}", remote_branch, fetch_commit.id()),
        //         )?;
        //         repo.set_head(&refname)?;
        //         repo.checkout_head(Some(
        //             git2::build::CheckoutBuilder::default()
        //                 .allow_conflicts(true)
        //                 .conflict_style_merge(true)
        //                 .force(),
        //         ))?;
        //     }
        // };
    } else if analysis.0.is_normal() {
        // do a normal merge
        let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
        normal_merge(&repo, &head_commit, &fetch_commit)?;
    } else {
        println!("Nothing to do...");
    }
    Ok(())
}
fn normal_merge(
    repo: &Repository,
    local: &git2::AnnotatedCommit,
    remote: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let local_tree = repo.find_commit(local.id())?.tree()?;
    let remote_tree = repo.find_commit(remote.id())?.tree()?;
    let ancestor = repo
        .find_commit(repo.merge_base(local.id(), remote.id())?)?
        .tree()?;
    let mut idx = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

    if idx.has_conflicts() {
        println!("Merge conficts detected...");
        repo.checkout_index(Some(&mut idx), None)?;
        return Ok(());
    }
    let result_tree = repo.find_tree(idx.write_tree_to(repo)?)?;
    // now create the merge commit
    let msg = format!("Merge: {} into {}", remote.id(), local.id());
    let sig = repo.signature()?;
    let local_commit = repo.find_commit(local.id())?;
    let remote_commit = repo.find_commit(remote.id())?;
    // Do our merge commit and set current branch head to that commit.
    let _merge_commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &msg,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;
    // Set working tree to match head.
    repo.checkout_head(None)?;
    Ok(())
}

fn fast_forward(
    repo: &Repository,
    lb: &mut git2::Reference,
    rc: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = match lb.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    println!("{}", msg);
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force(),
    ))?;
    Ok(())
}

fn push(repo: &Repository, url: &str) -> Result<(), git2::Error> {
    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(_) => repo.remote("origin", url)?,
    };
    remote.connect(Direction::Push)?;
    remote.push(&["refs/heads/master:refs/heads/master"], None)
}

fn execute_git<I, S, P>(p: P, args: I) -> Result<(), ()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    execute_git_fn(p, args)
}

fn execute_git_fn<I, S, P>(p: P, args: I) -> Result<(), ()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    println!("EXECUGE GIT");
    let output = Command::new("git").current_dir(p).args(args).output();
    println!("{:?}", output);
    // output.and_then(|output| {
    //     println!("{:?}", output)
    // };

    // output.and_then(|output| {
    //     if output.status.success() {
    //         if let Ok(message) = str::from_utf8(&output.stdout) {
    //             Ok(process(message))
    //         } else {
    //             Err(())
    //         }
    //     } else {
    //         if let Ok(stdout) = str::from_utf8(&output.stdout) {
    //             if let Ok(stderr) = str::from_utf8(&output.stderr) {
    //                 Err(())
    //             } else {
    //                 Err(())
    //             }
    //         } else {
    //             Err(())
    //         }
    //     }
    // });

    Ok(())
}
