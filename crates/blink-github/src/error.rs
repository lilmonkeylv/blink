#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("GitHub rejected the token; sign in again")]
    Unauthorized,
    #[error("OAuth error: {error}{}", description.as_deref().map(|d| format!(" ({d})")).unwrap_or_default())]
    OAuth { error: String, description: Option<String> },
    #[error("sign-in state mismatch")]
    StateMismatch,
    #[error("sign-in timed out")]
    Timeout,
    #[error("sign-in was cancelled in the browser")]
    Denied,
    #[error("GitHub API {status}: {message}")]
    Api { status: u16, message: String },
    #[error("rate limited until {reset}")]
    RateLimited { reset: i64 },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
