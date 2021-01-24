extern crate inotify;

use chrono::prelude::*;
use git2::{Commit, ObjectType, Oid, IndexAddOption, Repository, StatusOptions};
use inotify::{EventMask, WatchMask, Inotify};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

fn main() {
    let mut v: Vec<PathBuf> = Vec::new();

    v.push(PathBuf::from("/tmp/acp"));
    v.push(PathBuf::from("/tmp/acp2"));

    let mut inotify = Inotify::init()
        .expect("Failed to initialize inotify");

    let repositories = v.iter().fold(HashMap::new(), |mut acc, dir| {
        let wd = inotify
            .add_watch(
                dir,
                WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
            )
            .expect("Failed to add inotify watch");

        let repo = match Repository::open(dir) {
            Ok(repo) => repo,
            Err(e) => panic!("[git] failed to open: {}", e),
        };

        acc.insert(wd, repo);
        acc
    });

    println!("Watching current directory for activity...");

    let mut buffer = [0u8; 4096];
    loop {
        let events = inotify
            .read_events_blocking(&mut buffer)
            .expect("Failed to read inotify events");

        let mut modified_folders = HashSet::new();

        for event in events {
            modified_folders.insert(event.wd);
            if event.mask.contains(EventMask::CREATE) {
                if event.mask.contains(EventMask::ISDIR) {
                    println!("Directory created: {:?}", event.name);
                } else {
                    println!("File created: {:?}", event.name);
                }
            } else if event.mask.contains(EventMask::DELETE) {
                if event.mask.contains(EventMask::ISDIR) {
                    println!("Directory deleted: {:?}", event.name);
                } else {
                    println!("File deleted: {:?}", event.name);
                }
            } else if event.mask.contains(EventMask::MODIFY) {
                if event.mask.contains(EventMask::ISDIR) {
                    println!("Directory modified: {:?}", event.name);
                } else {
                    println!("File modified: {:?}", event.name);
                }
            }
        }

        println!("end of events for");

        for wd in modified_folders {
            let repo = repositories.get(&wd).unwrap();
            println!("is modified {}", is_modified(repo));
            if is_modified(repo) {
                add_and_commit(repo, format!("Auto-Commit at: {}", Local::now().format("%Y-%m-%d %H:%M:%S").to_string()));
            }
        }
    }
}

fn add_and_commit(repo: &Repository, message: String) -> Result<Oid, git2::Error> {
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None).expect("Cannot add file to git index");
    index.write().expect("Cannot write git index");

    let oid = index.write_tree()?;
    let signature = repo.signature().expect("Cannot fetch default signature");
    let parent_commit = find_last_commit(&repo)?;
    let tree = repo.find_tree(oid)?;
    repo.commit(Some("HEAD"), //  point HEAD to our new commit
                &signature, // author
                &signature, // committer
                &message[..], // commit message
                &tree, // tree
                &[&parent_commit]) // parents
}

fn find_last_commit(repo: &Repository) -> Result<Commit, git2::Error> {
    let obj = repo.head()?.resolve()?.peel(ObjectType::Commit)?;
    obj.into_commit().map_err(|_| git2::Error::from_str("Couldn't find commit"))
}

fn is_modified(repo: &Repository) -> bool {
    let mut opts = StatusOptions::new();
    opts
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut opts));

    !statuses.unwrap().is_empty()
}
