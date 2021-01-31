use chrono::prelude::*;
use git2::Repository;
mod git2_operation;
mod raw_git;

pub fn repo(folder: &str) -> Option<Repository> {
    match Repository::open(folder) {
        Ok(repo) => Some(repo),
        Err(_) => None,
    }
}

pub fn submit(repo: &Repository) -> Result<(), ()> {
    if git2_operation::is_modified(repo) {
        let message = format!("Auto-Commit at: {}", Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        git2_operation::add_all_and_commit(repo, message).map_err(|_| ())?;
        pull(repo)?;
        git2_operation::push(repo, "").map_err(|_| ())?;
        Ok(())
    } else {
        Ok(())
    }
}

fn pull(repo: &Repository) -> Result<(), ()> {
    let repo = raw_git::BinRepository::new(repo.workdir().unwrap());
    repo.pull_merge("origin", "master").map_err(|_| ())
}
