extern crate notify;

use chrono::prelude::*;
use git2::{Commit, IndexAddOption, ObjectType, Oid, Repository, StatusOptions};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

fn repo(folder: &str) -> Option<Repository> {
    match Repository::open(folder) {
        Ok(repo) => Some(repo),
        Err(_) => None,
    }
}

fn main() {
    let folders = vec!["/tmp/acp", "/tmp/acp2", "/tmp/acp3"];

    let repositories: HashMap<_, _> = folders
        .into_iter()
        .filter(|folder| Path::new(folder).exists())
        .filter_map(|folder| match repo(folder) {
            Some(repo) => Some((folder, repo)),
            None => None,
        })
        .collect();

    println!("folders: {:?}", repositories.keys());

    watch(repositories);
}

fn add_and_commit(repo: &Repository, message: String) -> Result<Oid, git2::Error> {
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
        Some("HEAD"), //  point HEAD to our new commit
        &signature,   // author
        &signature,   // committer
        &message[..], // commit message
        &tree,        // tree
        &[&parent_commit],
    ) // parents
}

fn find_last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
    let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
    obj.into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find commit"))
}

fn is_modified(repo: &Repository) -> bool {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut opts));

    !statuses.unwrap().is_empty()
}

fn watch(repositories: HashMap<&str, Repository>) -> notify::Result<()> {
    // Create a channel to receive the events.
    let (tx, rx) = channel();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(300))?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    for path in repositories.keys() {
        watcher.watch(path, RecursiveMode::Recursive)?;
        watcher.watch(path, RecursiveMode::Recursive)?;
    }

    // This is a simple loop, but you may want to use more complex logic here,
    // for example to handle I/O.
    loop {
        match rx.recv() {
            Ok(event) =>
            {
                match event {
                    DebouncedEvent::NoticeWrite(path)
                    | DebouncedEvent::NoticeRemove(path)
                    | DebouncedEvent::Create(path)
                    | DebouncedEvent::Write(path)
                    | DebouncedEvent::Chmod(path)
                    | DebouncedEvent::Remove(path)
                    | DebouncedEvent::Rename(_, path) => {
                        let repo = related_repository(path, &repositories).unwrap();
                        if is_modified(repo) {
                            add_and_commit(repo, format!("Auto-Commit at: {}", Local::now().format("%Y-%m-%d %H:%M:%S").to_string()));
                        }
                    },
                    _ => (),
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

fn related_repository<'a>(path: PathBuf, repositories: &'a HashMap<&'a str, Repository>) -> Option<&'a Repository> {
    let folder = repositories
        .keys()
        .find(|folder| path.starts_with(folder));

    match folder {
        Some(folder) => repositories.get(folder),
        None => None
    }
}
