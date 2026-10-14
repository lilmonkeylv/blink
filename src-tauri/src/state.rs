use std::collections::HashMap;
use std::sync::Arc;

use blink_core::{Engine, PoolConfig, RepoId};
use blink_git::{GitCli, Repository};
use blink_github::{Account, CheckState, Client};
use blink_store::{AppPaths, Cache, KeyringStore, SecretStore, Settings};
use blink_watch::RepoWatcher;
use parking_lot::{Mutex, RwLock};

use crate::accounts::{self, ACCOUNTS_KEY, LEGACY_ACCOUNT_KEY, REPO_CHOICE_KEY, REPO_DETECTED_KEY};

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

type Credentials = (String, String);

#[derive(Default)]
struct CredentialEntries {
    epoch: u64,
    keys: HashMap<String, Credentials>,
}

#[derive(Default)]
pub struct CredentialCache {
    inner: Mutex<CredentialEntries>,
}

impl CredentialCache {
    fn get_or_load(&self, key: &str, load: impl FnOnce() -> Option<Credentials>) -> Option<Credentials> {
        let epoch = {
            let inner = self.inner.lock();
            if let Some(known) = inner.keys.get(key) {
                return Some(known.clone());
            }
            inner.epoch
        };
        let loaded = load()?;
        let mut inner = self.inner.lock();
        if inner.epoch == epoch {
            inner.keys.insert(key.to_string(), loaded.clone());
        }
        Some(loaded)
    }

    fn forget(&self, host: &str) {
        let prefix = format!("{host}:");
        let mut inner = self.inner.lock();
        inner.epoch += 1;
        inner.keys.retain(|key, _| !key.starts_with(&prefix));
    }
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
    github_clients: Mutex<HashMap<String, Client>>,
    pub check_states: Mutex<HashMap<String, CheckState>>,
    credentials: CredentialCache,
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
        let cache = Cache::open(&paths.cache_db())?;
        if let Err(e) = accounts::migrate(&cache) {
            tracing::warn!(error = %e, "could not migrate the GitHub.com account");
        }
        Ok(Self {
            engine: Arc::new(Engine::new(pool)),
            cache,
            paths,
            settings: RwLock::new(settings),
            secrets: Box::new(KeyringStore),
            watchers: Mutex::new(HashMap::new()),
            oauth: OAuthConfig {
                client_id: env_or_build("BLINK_GITHUB_CLIENT_ID", option_env!("BLINK_GITHUB_CLIENT_ID")),
                client_secret: env_or_build("BLINK_GITHUB_CLIENT_SECRET", option_env!("BLINK_GITHUB_CLIENT_SECRET")),
            },
            github_clients: Mutex::new(HashMap::new()),
            check_states: Mutex::new(HashMap::new()),
            credentials: CredentialCache::default(),
        })
    }

    pub fn github_accounts(&self) -> Vec<Account> {
        self.cache.app_state(ACCOUNTS_KEY).ok().flatten().unwrap_or_default()
    }

    pub fn save_github_accounts(&self, list: &[Account]) -> Result<(), blink_store::Error> {
        self.cache.put_app_state(ACCOUNTS_KEY, &list)?;
        // builds without multi-account support read the single-account key
        self.cache.put_app_state(LEGACY_ACCOUNT_KEY, &list.first())?;
        self.cache.clear_repo_state(REPO_DETECTED_KEY)?;
        self.forget_credentials(GITHUB_HOST);
        Ok(())
    }

    pub fn account(&self) -> Option<Account> {
        self.github_accounts().into_iter().next()
    }

    pub fn repo_account_choice(&self, repo: &str) -> Option<String> {
        self.cache.repo_state::<Option<String>>(repo, REPO_CHOICE_KEY).ok().flatten().flatten()
    }

    pub fn set_repo_account_choice(&self, repo: &str, login: Option<&str>) -> Result<(), blink_store::Error> {
        self.cache.put_repo_state(repo, REPO_CHOICE_KEY, &login)
    }

    pub fn detected_repo_account(&self, repo: &str) -> Option<String> {
        self.cache.repo_state(repo, REPO_DETECTED_KEY).ok().flatten()
    }

    pub fn repo_account(&self, repo: Option<&str>) -> Option<Account> {
        let list = self.github_accounts();
        let chosen = repo.and_then(|r| self.repo_account_choice(r));
        let detected = repo.and_then(|r| self.detected_repo_account(r));
        accounts::pick(&list, chosen.as_deref(), detected.as_deref()).cloned()
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

    pub fn login_for(&self, host: &str, repo: Option<&str>) -> Option<String> {
        if host == GITHUB_HOST {
            return self.repo_account(repo).map(|a| a.login);
        }
        self.enterprise_accounts().into_iter().find(|a| a.host == host).map(|a| a.account.login)
    }

    fn credentials_as(&self, host: &str, login: &str) -> Option<Credentials> {
        let key = Self::host_token_key(host, login);
        self.credentials.get_or_load(&key, || {
            let token = self.secrets.get(&key).ok().flatten()?;
            Some((login.to_string(), token))
        })
    }

    pub fn forget_credentials(&self, host: &str) {
        self.credentials.forget(host);
        if host == GITHUB_HOST {
            self.github_clients.lock().clear();
        }
    }

    pub fn client_as(&self, host: &str, login: &str) -> Option<Client> {
        let (login, token) = self.credentials_as(host, login)?;
        if host != GITHUB_HOST {
            return Some(Client::new(api_base(host), token));
        }
        let mut clients = self.github_clients.lock();
        Some(clients.entry(login.to_ascii_lowercase()).or_insert_with(|| Client::github_com(token)).clone())
    }

    pub fn client_for(&self, host: &str, repo: Option<&str>) -> Option<Client> {
        self.client_as(host, &self.login_for(host, repo)?)
    }

    pub fn github(&self) -> Option<Client> {
        self.client_for(GITHUB_HOST, None)
    }

    pub fn git_cli(&self, repo: &Repository) -> GitCli {
        let sign = self.settings.read().git.sign_commits;
        let cli = repo.cli().with_signing(sign);
        let id = repo.workdir().to_string_lossy();
        self.authenticate(cli, crate::commands::repo::origin_url(repo).as_deref(), Some(&id))
    }

    pub fn authenticate(&self, cli: GitCli, remote: Option<&str>, repo: Option<&str>) -> GitCli {
        let Some(host) = remote.and_then(https_host) else { return cli };
        let Some(login) = self.login_for(&host, repo) else { return cli };
        self.authenticate_as(cli, remote, &login)
    }

    pub fn authenticate_as(&self, cli: GitCli, remote: Option<&str>, login: &str) -> GitCli {
        let Some(host) = remote.and_then(https_host) else { return cli };
        let Some((login, token)) = self.credentials_as(&host, login) else { return cli };
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

    fn test_state() -> AppState {
        let pool = PoolConfig { foreground_threads: 1, background_threads: 1, network_concurrency: 1 };
        AppState {
            engine: Arc::new(Engine::new(pool)),
            cache: Cache::in_memory().unwrap(),
            paths: AppPaths::rooted(std::path::Path::new("unused")),
            settings: RwLock::new(Settings::default()),
            secrets: Box::new(blink_store::MemoryStore::default()),
            watchers: Mutex::new(HashMap::new()),
            oauth: OAuthConfig { client_id: None, client_secret: None },
            github_clients: Mutex::new(HashMap::new()),
            check_states: Mutex::new(HashMap::new()),
            credentials: CredentialCache::default(),
        }
    }

    fn account(login: &str) -> Account {
        Account {
            login: login.into(),
            id: 1,
            name: None,
            email: None,
            avatar_url: String::new(),
            html_url: String::new(),
        }
    }

    #[test]
    fn each_repository_resolves_its_own_github_account() {
        let state = test_state();
        state.secrets.set(&AppState::token_key("octo"), "t1").unwrap();
        state.secrets.set(&AppState::token_key("work"), "t2").unwrap();
        state.save_github_accounts(&[account("octo"), account("work")]).unwrap();
        let login = |repo: &str| state.login_for(GITHUB_HOST, Some(repo));

        assert_eq!(state.login_for(GITHUB_HOST, None).as_deref(), Some("octo"));
        assert_eq!(login("/r").as_deref(), Some("octo"));
        state.set_repo_account_choice("/r", Some("work")).unwrap();
        assert_eq!(login("/r").as_deref(), Some("work"));
        assert_eq!(state.credentials_as(GITHUB_HOST, "work").unwrap(), ("work".to_string(), "t2".to_string()));

        state.cache.put_repo_state("/d", REPO_DETECTED_KEY, &"work").unwrap();
        assert_eq!(login("/d").as_deref(), Some("work"));
        state.save_github_accounts(&[account("octo"), account("work")]).unwrap();
        assert_eq!(login("/d").as_deref(), Some("octo"));

        state.save_github_accounts(&[account("octo")]).unwrap();
        assert_eq!(login("/r").as_deref(), Some("octo"));
        state.set_repo_account_choice("/r", None).unwrap();
        assert_eq!(state.repo_account_choice("/r"), None);

        let legacy: Option<Option<Account>> = state.cache.app_state(LEGACY_ACCOUNT_KEY).unwrap();
        assert_eq!(legacy.flatten().map(|a| a.login).as_deref(), Some("octo"));
    }

    #[test]
    fn enterprise_hosts_keep_their_single_account() {
        let state = test_state();
        state.save_github_accounts(&[account("octo"), account("work")]).unwrap();
        state.set_repo_account_choice("/r", Some("work")).unwrap();
        let host = "ghe.acme.com".to_string();
        state.save_enterprise_accounts(&[EnterpriseAccount { host: host.clone(), account: account("me") }]).unwrap();
        state.secrets.set(&AppState::host_token_key(&host, "me"), "t3").unwrap();
        assert_eq!(state.login_for(&host, Some("/r")).as_deref(), Some("me"));
        assert!(state.client_for(&host, Some("/r")).is_some());
        assert!(state.client_for(GITHUB_HOST, Some("/r")).is_none());
    }

    #[test]
    fn only_github_https_gets_token() {
        assert!(is_github_https("https://github.com/a/b.git"));
        assert!(!is_github_https("https://gitlab.com/a/b.git"));
        assert!(!is_github_https("git@github.com:a/b.git"));
        assert!(!is_github_https("https://github.com.evil.io/a/b.git"));
    }

    #[test]
    fn credentials_are_cached_until_forgotten() {
        let cache = CredentialCache::default();
        let loads = std::cell::Cell::new(0);
        let load = || {
            loads.set(loads.get() + 1);
            Some(("me".to_string(), format!("token{}", loads.get())))
        };
        assert_eq!(cache.get_or_load("github.com:me", load).unwrap().1, "token1");
        assert_eq!(cache.get_or_load("github.com:me", load).unwrap().1, "token1");
        assert_eq!(loads.get(), 1);
        assert_eq!(cache.get_or_load("ghe.acme.com:me", || None), None);
        assert_eq!(cache.get_or_load("ghe.acme.com:me", || None), None);
        cache.forget("github.com");
        assert_eq!(cache.get_or_load("github.com:me", load).unwrap().1, "token2");
    }

    #[test]
    fn a_sign_out_during_a_load_is_not_undone() {
        let cache = CredentialCache::default();
        let stale = cache.get_or_load("github.com:me", || {
            cache.forget("github.com");
            Some(("me".to_string(), "stale".to_string()))
        });
        assert_eq!(stale.unwrap().1, "stale");
        assert_eq!(cache.get_or_load("github.com:me", || None), None);
    }

    #[test]
    fn forgetting_a_host_keeps_other_hosts() {
        let cache = CredentialCache::default();
        let creds = |token: &str| Some(("me".to_string(), token.to_string()));
        cache.get_or_load("github.com:me", || creds("a"));
        cache.get_or_load("github.com:work", || creds("b"));
        cache.get_or_load("github.company.io:me", || creds("c"));
        cache.forget("github.com");
        assert_eq!(cache.get_or_load("github.com:me", || None), None);
        assert_eq!(cache.get_or_load("github.com:work", || None), None);
        assert_eq!(cache.get_or_load("github.company.io:me", || None).unwrap().1, "c");
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
