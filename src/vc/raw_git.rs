use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use thiserror::Error;
use std::result::Result as stdResult;

pub type Result<A> = stdResult<A, GitError>;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Unable to execute git process")]
    Execution,
    #[error("Unable to decode error from git executable")]
    Undecodable,
    #[error("git failed with the following stdout: {stdout}")]
    GitError{
        stdout: String,
        stderr: String
    },
}

pub struct BinRepository {
    location: PathBuf,
}

impl BinRepository {
    pub fn new<P: AsRef<Path>>(p: P) -> BinRepository {
        let p = p.as_ref();
        BinRepository {
            location: PathBuf::from(p),
        }
    }

    pub fn pull_merge(&self, upstream: &str, upstream_branch: &str) -> Result<()> {
        execute_git(
            &self.location,
            &["pull", upstream, upstream_branch],
        )
    }
}

fn execute_git<I, S, P>(p: P, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
{
    execute_git_fn(p, args, |_| ())
}

fn execute_git_fn<I, S, P, F, R>(p: P, args: I, process: F) -> Result<R>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    P: AsRef<Path>,
    F: Fn(&str) -> R,
{
    let output = Command::new("git").current_dir(p).args(args).output();

    output.map_err(|_| GitError::Execution).and_then(|output| {
        if output.status.success() {
            if let Ok(message) = str::from_utf8(&output.stdout) {
                Ok(process(message))
            } else {
                Err(GitError::Undecodable)
            }
        } else {
            if let Ok(stdout) = str::from_utf8(&output.stdout) {
                if let Ok(stderr) = str::from_utf8(&output.stderr) {
                    Err(GitError::GitError {
                        stdout: stdout.to_owned(),
                        stderr: stderr.to_owned(),
                    })
                } else {
                    Err(GitError::Undecodable)
                }
            } else {
                Err(GitError::Undecodable)
            }
        }
    })
}
