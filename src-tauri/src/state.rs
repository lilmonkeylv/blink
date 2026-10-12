use std::collections::HashMap;
use std::sync::Arc;

use blink_core::{Engine, PoolConfig, RepoId};
use blink_git::{GitCli, Repository};
use blink_github::{Account, CheckState, Client};
use blink_store::{AppPaths, Cache, KeyringStore, SecretStore, Settings};
use blink_watch::RepoWatcher;
use parking_lot::{Mutex, RwLock};

pub const ACCOUNT_KEY: &str = "account.github.com";

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
        format!("github.com:{login}")
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
        let Some((account, token)) = self.token() else { return cli };
        if !remote.is_some_and(is_github_https) {
            return cli;
        }
        cli.with_config("credential.https://github.com.helper", "")
            .with_config("credential.https://github.com.helper", CREDENTIAL_HELPER)
            .with_env("BLINK_GIT_USER", account.login)
            .with_env("BLINK_GIT_TOKEN", token)
    }
}

const CREDENTIAL_HELPER: &str =
    "!f() { test \"$1\" = get || exit 0; echo \"username=$BLINK_GIT_USER\"; echo \"password=$BLINK_GIT_TOKEN\"; }; f";

pub fn is_github_https(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| u.scheme() == "https" && u.host_str() == Some("github.com"))
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
}
