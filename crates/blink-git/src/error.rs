use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a git repository: {0}")]
    NotARepository(String),
    #[error("repository has no working tree")]
    Bare,
    #[error("{0}")]
    Git(String),
    #[error("git {command} failed: {stderr}")]
    Cli { command: String, stderr: String, code: Option<i32> },
    #[error("git executable not found")]
    GitNotFound,
    #[error("operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) trait ResultExt<T> {
    fn git(self) -> Result<T>;
}

impl<T, E: Display> ResultExt<T> for std::result::Result<T, E> {
    fn git(self) -> Result<T> {
        self.map_err(|e| Error::Git(e.to_string()))
    }
}
