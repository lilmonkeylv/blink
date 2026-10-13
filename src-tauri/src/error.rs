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

    fn sso(url: String) -> Self {
        Self { kind: "sso", message: "This organization requires single sign-on".into(), detail: Some(url) }
    }
}

const SSO_FALLBACK_URL: &str = "https://github.com/settings/applications";

fn mentions_sso(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("saml sso") || lower.contains("re-authorize the oauth application")
}

fn sso_link(stderr: &str) -> String {
    stderr
        .split(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '<' | '>'))
        .find(|word| word.starts_with("https://") && word.contains("/sso"))
        .map(|word| word.trim_end_matches(['.', ',', ')']).to_string())
        .unwrap_or_else(|| SSO_FALLBACK_URL.to_string())
}

impl From<blink_git::Error> for CommandError {
    fn from(e: blink_git::Error) -> Self {
        match e {
            blink_git::Error::Cli { stderr, .. } if mentions_sso(&stderr) => Self::sso(sso_link(&stderr)),
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
        if let blink_github::Error::SsoRequired { url } = e {
            return Self::sso(url);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn push_failure(stderr: &str) -> CommandError {
        blink_git::Error::Cli { command: "push".into(), stderr: stderr.into(), code: Some(128) }.into()
    }

    #[test]
    fn git_sso_failure_links_to_authorization() {
        let e = push_failure(
            "remote: The 'acme' organization has enabled or enforced SAML SSO.\nremote: To access this repository, visit https://github.com/enterprises/acme/sso?authorization_request=AB12 and try again.\nfatal: unable to access 'https://github.com/acme/app.git/': 403",
        );
        assert_eq!(e.kind, "sso");
        assert_eq!(e.detail.as_deref(), Some("https://github.com/enterprises/acme/sso?authorization_request=AB12"));
    }

    #[test]
    fn git_sso_failure_without_link_falls_back() {
        let e = push_failure("remote: To access this repository, you must re-authorize the OAuth Application 'Blink'.");
        assert_eq!(e.kind, "sso");
        assert_eq!(e.detail.as_deref(), Some(SSO_FALLBACK_URL));
    }

    #[test]
    fn other_git_failures_stay_git() {
        assert_eq!(push_failure("fatal: Authentication failed").kind, "git");
    }

    #[test]
    fn api_sso_error_carries_url() {
        let url = "https://github.com/orgs/acme/sso?authorization_request=AB12".to_string();
        let e = CommandError::from(blink_github::Error::SsoRequired { url: url.clone() });
        assert_eq!((e.kind, e.detail), ("sso", Some(url)));
    }
}
