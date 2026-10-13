use std::collections::HashMap;
use std::sync::Arc;

use blink_core::{Engine, PoolConfig, RepoId};
use blink_git::{GitCli, Repository};
use blink_github::{Account, CheckState, Client};
use blink_store::{AppPaths, Cache, KeyringStore, SecretStore, Settings};
use blink_watch::RepoWatcher;
use parking_lot::{Mutex, RwLock};

pub const ACCOUNT_KEY: &str = "account.github.com";
pub const ENTERPRISE_KEY: &str = "accounts.enterprise";
pub const GITHUB_HOST: &str = "github.com";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterpriseAccount {
    pub host: String,
    pub account: Account,
}

pub fn normalize_host(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('/');
    let with_scheme = if trimmed.contains("://") { trimmed.to_string() } else { format!("https://{trimmed}") };
    let url = url::Url::parse(&with_scheme).ok()?;
    url.host_str().map(|h| h.to_ascii_lowercase())
}

pub fn api_base(host: &str) -> String {
    match host {
        GITHUB_HOST => "https://api.github.com".to_string(),
        h if h.ends_with(".ghe.com") => format!("https://api.{h}"),
        h => format!("https://{h}/api/v3"),
    }
}

pub fn https_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    (parsed.scheme() == "https").then(|| parsed.host_str().map(str::to_ascii_lowercase)).flatten()
}

pub struct OAuthConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

pub struct AppState {
    pub engine: Arc<Engine>,
    pub cache: Cache,
    pub paths: AppPaths,
    pub settings: RwLock<Settings>,
    pub secrets: Box<dyn SecretStore>,
    pub watchers: Mutex<HashMap<RepoId, RepoWatcher>>,
    pub oauth: OAuthConfig,
    pub github: Mutex<Option<(String, Client)>>,
    pub check_states: Mutex<HashMap<String, CheckState>>,
}

fn env_or_build(name: &str, built: Option<&'static str>) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty()).or_else(|| built.map(str::to_string)).filter(|v| !v.is_empty())
}

fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env").or_else(|_| std::fs::read_to_string("../.env")) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && std::env::var_os(key.trim()).is_none()
        {
            // SAFETY: called once at startup before any other threads read the environment.
            unsafe { std::env::set_var(key.trim(), value.trim().trim_matches('"')) };
        }
    }
}

impl AppState {
    pub fn initialize() -> Result<Self, blink_store::Error> {
        if cfg!(debug_assertions) {
            load_dotenv();
        }
        let paths = AppPaths::system()?;
        paths.ensure()?;
        let settings = Settings::load(&paths.settings_file()).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "settings unreadable, using defaults");
            Settings::default()
        });
        let mut pool = PoolConfig::default();
        if let Some(n) = settings.advanced.foreground_threads {
            pool.foreground_threads = n.max(1);
        }
        if let Some(n) = settings.advanced.background_threads {
            pool.background_threads = n.max(1);
        }
        Ok(Self {
            engine: Arc::new(Engine::new(pool)),
            cache: Cache::open(&paths.cache_db())?,
            paths,
            settings: RwLock::new(settings),
            secrets: Box::new(KeyringStore),
            watchers: Mutex::new(HashMap::new()),
            oauth: OAuthConfig {
                client_id: env_or_build("BLINK_GITHUB_CLIENT_ID", option_env!("BLINK_GITHUB_CLIENT_ID")),
                client_secret: env_or_build("BLINK_GITHUB_CLIENT_SECRET", option_env!("BLINK_GITHUB_CLIENT_SECRET")),
            },
            github: Mutex::new(None),
            check_states: Mutex::new(HashMap::new()),
        })
    }

    pub fn account(&self) -> Option<Account> {
        self.cache.app_state(ACCOUNT_KEY).ok().flatten()
    }

    pub fn token_key(login: &str) -> String {
        Self::host_token_key(GITHUB_HOST, login)
    }

    pub fn host_token_key(host: &str, login: &str) -> String {
        format!("{host}:{login}")
    }

    pub fn enterprise_accounts(&self) -> Vec<EnterpriseAccount> {
        self.cache.app_state(ENTERPRISE_KEY).ok().flatten().unwrap_or_default()
    }

    pub fn save_enterprise_accounts(&self, accounts: &[EnterpriseAccount]) -> Result<(), blink_store::Error> {
        self.cache.put_app_state(ENTERPRISE_KEY, &accounts)
    }

    pub fn credentials_for_host(&self, host: &str) -> Option<(String, String)> {
        if host == GITHUB_HOST {
            return self.token().map(|(account, token)| (account.login, token));
        }
        let account = self.enterprise_accounts().into_iter().find(|a| a.host == host)?;
        let token = self.secrets.get(&Self::host_token_key(host, &account.account.login)).ok().flatten()?;
        Some((account.account.login, token))
    }

    pub fn client_for_host(&self, host: &str) -> Option<Client> {
        if host == GITHUB_HOST {
            return self.github();
        }
        let (_, token) = self.credentials_for_host(host)?;
        Some(Client::new(api_base(host), token))
    }

    pub fn token(&self) -> Option<(Account, String)> {
        let account = self.account()?;
        let token = self.secrets.get(&Self::token_key(&account.login)).ok().flatten()?;
        Some((account, token))
    }

    pub fn github(&self) -> Option<Client> {
        let (account, token) = self.token()?;
        let mut cached = self.github.lock();
        if let Some((login, client)) = cached.as_ref()
            && *login == account.login
        {
            return Some(client.clone());
        }
        let client = Client::github_com(token);
        *cached = Some((account.login, client.clone()));
        Some(client)
    }

    pub fn git_cli(&self, repo: &Repository) -> GitCli {
        let sign = self.settings.read().git.sign_commits;
        let cli = repo.cli().with_signing(sign);
        self.authenticate(cli, crate::commands::repo::origin_url(repo).as_deref())
    }

    pub fn authenticate(&self, cli: GitCli, remote: Option<&str>) -> GitCli {
        let Some(host) = remote.and_then(https_host) else { return cli };
        let Some((login, token)) = self.credentials_for_host(&host) else { return cli };
        let key = format!("credential.https://{host}.helper");
        cli.with_config(&key, "")
            .with_config(&key, CREDENTIAL_HELPER)
            .with_env("BLINK_GIT_USER", login)
            .with_env("BLINK_GIT_TOKEN", token)
    }
}

const CREDENTIAL_HELPER: &str =
    "!f() { test \"$1\" = get || exit 0; echo \"username=$BLINK_GIT_USER\"; echo \"password=$BLINK_GIT_TOKEN\"; }; f";

#[cfg(test)]
fn is_github_https(url: &str) -> bool {
    https_host(url).as_deref() == Some(GITHUB_HOST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_github_https_gets_token() {
        assert!(is_github_https("https://github.com/a/b.git"));
        assert!(!is_github_https("https://gitlab.com/a/b.git"));
        assert!(!is_github_https("git@github.com:a/b.git"));
        assert!(!is_github_https("https://github.com.evil.io/a/b.git"));
    }

    #[test]
    fn hosts_and_api_bases() {
        assert_eq!(normalize_host("https://GHE.acme.com/").as_deref(), Some("ghe.acme.com"));
        assert_eq!(normalize_host("ghe.acme.com").as_deref(), Some("ghe.acme.com"));
        assert_eq!(api_base("github.com"), "https://api.github.com");
        assert_eq!(api_base("ghe.acme.com"), "https://ghe.acme.com/api/v3");
        assert_eq!(api_base("acme.ghe.com"), "https://api.acme.ghe.com");
        assert_eq!(https_host("https://ghe.acme.com/a/b.git").as_deref(), Some("ghe.acme.com"));
        assert_eq!(https_host("git@ghe.acme.com:a/b.git"), None);
    }
}
