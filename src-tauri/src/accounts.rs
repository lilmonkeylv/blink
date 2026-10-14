use blink_git::Repository;
use blink_github::Account;
use blink_store::Cache;

use crate::commands::github::hosted_repo;
use crate::state::{AppState, GITHUB_HOST};

pub const LEGACY_ACCOUNT_KEY: &str = "account.github.com";
pub const ACCOUNTS_KEY: &str = "accounts.github.com";
pub const REPO_CHOICE_KEY: &str = "githubAccount";
pub const REPO_DETECTED_KEY: &str = "githubAccountDetected";

pub fn same_login(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

pub fn migrate(cache: &Cache) -> blink_store::Result<()> {
    if !matches!(cache.app_state::<serde_json::Value>(ACCOUNTS_KEY), Ok(None)) {
        return Ok(());
    }
    let legacy = cache.app_state::<Option<Account>>(LEGACY_ACCOUNT_KEY).ok().flatten().flatten();
    cache.put_app_state(ACCOUNTS_KEY, &Vec::from_iter(legacy))
}

pub fn upsert(accounts: &mut Vec<Account>, account: Account) {
    match accounts.iter_mut().find(|a| same_login(&a.login, &account.login)) {
        Some(existing) => *existing = account,
        None => accounts.push(account),
    }
}

pub fn make_primary(accounts: &mut Vec<Account>, login: &str) -> bool {
    let Some(index) = accounts.iter().position(|a| same_login(&a.login, login)) else { return false };
    let account = accounts.remove(index);
    accounts.insert(0, account);
    true
}

pub fn pick<'a>(accounts: &'a [Account], chosen: Option<&str>, detected: Option<&str>) -> Option<&'a Account> {
    let find = |login: &str| accounts.iter().find(|a| same_login(&a.login, login));
    chosen.and_then(find).or_else(|| detected.and_then(find)).or_else(|| accounts.first())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Push,
    Read,
    Denied,
}

pub fn best_access(results: &[(String, Access)]) -> Option<&str> {
    let first = |wanted: Access| results.iter().find(|(_, access)| *access == wanted).map(|(login, _)| login.as_str());
    first(Access::Push).or_else(|| first(Access::Read))
}

async fn probe(state: &AppState, accounts: &[Account], owner: &str, name: &str) -> Option<String> {
    let mut results = Vec::with_capacity(accounts.len());
    for account in accounts {
        let Some(gh) = state.client_as(GITHUB_HOST, &account.login) else { continue };
        let access = match gh.repo(owner, name).await {
            Ok(repo) if repo.permissions.is_some_and(|p| p.push) => Access::Push,
            Ok(_) => Access::Read,
            Err(_) => Access::Denied,
        };
        results.push((account.login.clone(), access));
        if access == Access::Push {
            break;
        }
    }
    best_access(&results).map(str::to_string)
}

pub async fn resolve(state: &AppState, id: &str, repo: &Repository) -> Option<Account> {
    let accounts = state.github_accounts();
    let undecided =
        accounts.len() > 1 && state.repo_account_choice(id).is_none() && state.detected_repo_account(id).is_none();
    if undecided
        && let Some(hosted) = hosted_repo(state, repo, None).filter(|h| h.host == GITHUB_HOST)
        && let Some(login) = probe(state, &accounts, &hosted.owner, &hosted.name).await
    {
        let _ = state.cache.put_repo_state(id, REPO_DETECTED_KEY, &login);
    }
    state.repo_account(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(login: &str) -> Account {
        Account {
            login: login.into(),
            id: login.len() as u64,
            name: None,
            email: None,
            avatar_url: String::new(),
            html_url: String::new(),
        }
    }

    fn logins(accounts: &[Account]) -> Vec<&str> {
        accounts.iter().map(|a| a.login.as_str()).collect()
    }

    #[test]
    fn the_single_legacy_account_becomes_the_primary() {
        let cache = Cache::in_memory().unwrap();
        cache.put_app_state(LEGACY_ACCOUNT_KEY, &account("octo")).unwrap();
        migrate(&cache).unwrap();
        let migrated: Vec<Account> = cache.app_state(ACCOUNTS_KEY).unwrap().unwrap();
        assert_eq!(logins(&migrated), ["octo"]);

        cache.put_app_state(ACCOUNTS_KEY, &vec![account("work"), account("octo")]).unwrap();
        migrate(&cache).unwrap();
        let kept: Vec<Account> = cache.app_state(ACCOUNTS_KEY).unwrap().unwrap();
        assert_eq!(logins(&kept), ["work", "octo"]);
    }

    #[test]
    fn signed_out_or_fresh_installs_migrate_to_no_accounts() {
        let signed_out = Cache::in_memory().unwrap();
        signed_out.put_app_state(LEGACY_ACCOUNT_KEY, &Option::<Account>::None).unwrap();
        migrate(&signed_out).unwrap();
        assert_eq!(signed_out.app_state::<Vec<Account>>(ACCOUNTS_KEY).unwrap(), Some(vec![]));

        let fresh = Cache::in_memory().unwrap();
        migrate(&fresh).unwrap();
        assert_eq!(fresh.app_state::<Vec<Account>>(ACCOUNTS_KEY).unwrap(), Some(vec![]));
    }

    #[test]
    fn signing_in_again_replaces_the_same_login_in_place() {
        let mut accounts = vec![account("octo"), account("work")];
        let mut renamed = account("Octo");
        renamed.name = Some("Octo Cat".into());
        upsert(&mut accounts, renamed);
        assert_eq!(logins(&accounts), ["Octo", "work"]);
        upsert(&mut accounts, account("third"));
        assert_eq!(logins(&accounts), ["Octo", "work", "third"]);
    }

    #[test]
    fn making_an_account_primary_moves_it_first() {
        let mut accounts = vec![account("octo"), account("work"), account("third")];
        assert!(make_primary(&mut accounts, "THIRD"));
        assert_eq!(logins(&accounts), ["third", "octo", "work"]);
        assert!(!make_primary(&mut accounts, "missing"));
    }

    #[test]
    fn repositories_use_their_choice_then_detection_then_the_primary() {
        let accounts = vec![account("octo"), account("work")];
        assert_eq!(pick(&accounts, None, None).unwrap().login, "octo");
        assert_eq!(pick(&accounts, None, Some("work")).unwrap().login, "work");
        assert_eq!(pick(&accounts, Some("octo"), Some("work")).unwrap().login, "octo");
        assert_eq!(pick(&accounts, Some("removed"), None).unwrap().login, "octo");
        assert_eq!(pick(&accounts, Some("removed"), Some("WORK")).unwrap().login, "work");
        assert!(pick(&[], Some("octo"), None).is_none());
    }

    #[test]
    fn detection_prefers_push_access_then_the_first_reader() {
        let r = |pairs: &[(&str, Access)]| pairs.iter().map(|(l, a)| (l.to_string(), *a)).collect::<Vec<_>>();
        assert_eq!(best_access(&r(&[("octo", Access::Read), ("work", Access::Push)])), Some("work"));
        assert_eq!(best_access(&r(&[("octo", Access::Denied), ("work", Access::Read)])), Some("work"));
        assert_eq!(best_access(&r(&[("octo", Access::Read), ("work", Access::Read)])), Some("octo"));
        assert_eq!(best_access(&r(&[("octo", Access::Denied)])), None);
    }
}
