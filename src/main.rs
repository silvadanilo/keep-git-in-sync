extern crate notify;

use git2::Repository;
use log::debug;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;
mod vc;

fn main() -> Result<(), notify::Error> {
    env_logger::init();

    let folders = vec!["/tmp/acp", "/tmp/acp2", "/tmp/acp3", "/tmp/foo"];

    let repositories: HashMap<&str, Repository> = folders
        .into_iter()
        .filter(|folder| Path::new(folder).exists())
        .filter_map(|folder| match vc::repo(folder) {
            Some(repo) => Some((folder, repo)),
            None => None,
        })
        .collect();

    debug!("listening on folders: {:?}", repositories.keys());

    watch(repositories)
}

fn watch(repositories: HashMap<&str, Repository>) -> notify::Result<()> {
    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_millis(100))?;

    for path in repositories.keys() {
        watcher.watch(path, RecursiveMode::Recursive)?;
    }

    loop {
        if let Ok(event) = rx.recv() {
            if let Some(path) = changed_path(event) {
                let repo = related_repository(path, &repositories).unwrap();
                vc::submit(repo).unwrap_or(());
            }
        }
    }
}

fn related_repository<'a>(
    path: PathBuf,
    repositories: &'a HashMap<&'a str, Repository>,
) -> Option<&'a Repository> {
    let folder = repositories.keys().find(|folder| path.starts_with(folder));

    match folder {
        Some(folder) => repositories.get(folder),
        None => None,
    }
}

fn changed_path(event: DebouncedEvent) -> Option<PathBuf> {
    match event {
        DebouncedEvent::NoticeWrite(path)
        | DebouncedEvent::NoticeRemove(path)
        | DebouncedEvent::Create(path)
        | DebouncedEvent::Write(path)
        | DebouncedEvent::Chmod(path)
        | DebouncedEvent::Remove(path)
        | DebouncedEvent::Rename(_, path) => Some(path),
        _ => None,
    }.and_then(|path| {
        let is_git = path.components()
            .any(|component| component == Component::Normal(OsStr::new(".git")));

        match is_git {
            true => None,
            false => Some(path)
        }
    })
}
