#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Git(#[from] blink_git::Error),
    #[error("repository not open: {0}")]
    UnknownRepo(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("worker pool shut down")]
    PoolClosed,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
