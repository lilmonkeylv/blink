use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub kind: &'static str,
    pub message: String,
    pub detail: Option<String>,
}

pub type CmdResult<T> = Result<T, CommandError>;

impl CommandError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), detail: None }
    }
}

impl From<blink_git::Error> for CommandError {
    fn from(e: blink_git::Error) -> Self {
        match e {
            blink_git::Error::Cli { command, stderr, .. } => {
                Self { kind: "git", message: format!("git {command} failed"), detail: Some(stderr) }
            }
            blink_git::Error::NotARepository(path) => Self::new("notARepository", path),
            blink_git::Error::GitNotFound => Self::new("gitNotFound", "Git is not installed or not on PATH"),
            other => Self::new("git", blink_git::redact(&other.to_string())),
        }
    }
}

impl From<blink_core::Error> for CommandError {
    fn from(e: blink_core::Error) -> Self {
        match e {
            blink_core::Error::Git(g) => g.into(),
            blink_core::Error::Cancelled => Self::new("cancelled", "superseded"),
            other => Self::new("core", other.to_string()),
        }
    }
}

impl From<blink_store::Error> for CommandError {
    fn from(e: blink_store::Error) -> Self {
        Self::new("store", e.to_string())
    }
}

impl From<blink_github::Error> for CommandError {
    fn from(e: blink_github::Error) -> Self {
        let kind = match e {
            blink_github::Error::Unauthorized => "unauthorized",
            blink_github::Error::Timeout => "timeout",
            blink_github::Error::Denied => "denied",
            _ => "github",
        };
        Self::new(kind, e.to_string())
    }
}

impl From<blink_watch::Error> for CommandError {
    fn from(e: blink_watch::Error) -> Self {
        Self::new("watch", e.to_string())
    }
}

impl From<tauri::Error> for CommandError {
    fn from(e: tauri::Error) -> Self {
        Self::new("tauri", e.to_string())
    }
}
