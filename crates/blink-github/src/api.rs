use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::oauth::http_client;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
    pub id: u64,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
    #[serde(default)]
    pub html_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    pub login: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: Owner,
    pub private: bool,
    pub fork: bool,
    pub clone_url: String,
    pub ssh_url: String,
    pub html_url: String,
    pub default_branch: Option<String>,
    pub description: Option<String>,
    pub pushed_at: Option<String>,
    #[serde(default)]
    pub parent: Option<Box<Repo>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Org {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRef {
    #[serde(rename = "ref")]
    pub name: String,
    pub sha: String,
    pub repo: Option<PullRepo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRepo {
    pub full_name: String,
    pub clone_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub html_url: String,
    pub state: String,
    #[serde(default)]
    pub draft: bool,
    pub user: Owner,
    pub head: PullRef,
    pub base: PullRef,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckSuiteRef {
    pub id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: Option<String>,
    pub check_suite: Option<CheckSuiteRef>,
}

#[derive(Deserialize)]
struct CheckRuns {
    check_runs: Vec<CheckRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitStatus {
    pub context: String,
    pub state: String,
    pub target_url: Option<String>,
}

#[derive(Deserialize)]
struct CombinedStatus {
    statuses: Vec<CommitStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckState {
    Success,
    Failure,
    Pending,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecksSummary {
    pub state: CheckState,
    pub total: usize,
    pub failed: usize,
    pub pending: usize,
    pub runs: Vec<CheckRun>,
    pub statuses: Vec<CommitStatus>,
}

pub fn summarize_checks(runs: Vec<CheckRun>, statuses: Vec<CommitStatus>) -> ChecksSummary {
    let failed_conclusions = ["failure", "timed_out", "cancelled", "action_required", "startup_failure"];
    let failed =
        runs.iter().filter(|r| r.conclusion.as_deref().is_some_and(|c| failed_conclusions.contains(&c))).count()
            + statuses.iter().filter(|s| s.state == "failure" || s.state == "error").count();
    let pending = runs.iter().filter(|r| r.status != "completed").count()
        + statuses.iter().filter(|s| s.state == "pending").count();
    let total = runs.len() + statuses.len();
    let state = match (total, failed, pending) {
        (0, ..) => CheckState::None,
        (_, f, _) if f > 0 => CheckState::Failure,
        (_, _, p) if p > 0 => CheckState::Pending,
        _ => CheckState::Success,
    };
    ChecksSummary { state, total, failed, pending, runs, statuses }
}

pub fn parse_remote(url: &str) -> Option<(String, String, String)> {
    let url = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let (host, path) = if let Some(rest) = url.strip_prefix("git@") {
        rest.split_once(':')?
    } else {
        let parsed = url::Url::parse(url).ok()?;
        let host = parsed.host_str()?.to_string();
        let path = parsed.path().trim_start_matches('/').to_string();
        let (owner, name) = path.split_once('/')?;
        return Some((host, owner.to_string(), name.to_string()));
    };
    let (owner, name) = path.split_once('/')?;
    Some((host.to_string(), owner.to_string(), name.to_string()))
}

type CachedResponse = (String, Arc<Vec<u8>>);

#[derive(Debug, Clone)]
pub struct Client {
    api_base: String,
    token: String,
    http: reqwest::Client,
    etags: Arc<Mutex<HashMap<String, CachedResponse>>>,
}

#[derive(Deserialize)]
struct ApiMessage {
    message: String,
}

impl Client {
    pub fn new(api_base: impl Into<String>, token: impl Into<String>) -> Self {
        Self { api_base: api_base.into(), token: token.into(), http: http_client(), etags: Default::default() }
    }

    pub fn github_com(token: impl Into<String>) -> Self {
        Self::new("https://api.github.com", token)
    }

    fn request(&self, method: Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    async fn check(response: reqwest::Response) -> Result<reqwest::Response> {
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(Error::Unauthorized);
        }
        if status == StatusCode::FORBIDDEN
            && let Some(url) = response.headers().get("x-github-sso").and_then(|v| v.to_str().ok()).and_then(sso_url)
        {
            return Err(Error::SsoRequired { url });
        }
        if (status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS)
            && response.headers().get("x-ratelimit-remaining").is_some_and(|v| v == "0")
        {
            let reset = response
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or_default();
            return Err(Error::RateLimited { reset });
        }
        if !status.is_success() && status != StatusCode::NOT_MODIFIED {
            let message = response.json::<ApiMessage>().await.map(|m| m.message).unwrap_or_default();
            return Err(Error::Api { status: status.as_u16(), message });
        }
        Ok(response)
    }

    async fn get_url<T: DeserializeOwned>(&self, url: &str) -> Result<(T, Option<String>)> {
        let mut req = self.request(Method::GET, url);
        let cached = self.etags.lock().get(url).cloned();
        if let Some((etag, _)) = &cached {
            req = req.header("If-None-Match", etag);
        }
        let response = Self::check(req.send().await?).await?;
        let next = response.headers().get("link").and_then(|v| v.to_str().ok()).and_then(next_link);
        if response.status() == StatusCode::NOT_MODIFIED
            && let Some((_, body)) = cached
        {
            return Ok((
                serde_json::from_slice(&body).map_err(|e| Error::Api { status: 304, message: e.to_string() })?,
                next,
            ));
        }
        let etag = response.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_string);
        let body = response.bytes().await?.to_vec();
        let value = serde_json::from_slice(&body).map_err(|e| Error::Api { status: 200, message: e.to_string() })?;
        if let Some(etag) = etag {
            self.etags.lock().insert(url.to_string(), (etag, Arc::new(body)));
        }
        Ok((value, next))
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        Ok(self.get_url(&format!("{}{path}", self.api_base)).await?.0)
    }

    async fn get_all<T: DeserializeOwned>(&self, path: &str, max_pages: usize) -> Result<Vec<T>> {
        let mut url = Some(format!("{}{path}", self.api_base));
        let mut out = Vec::new();
        let mut pages = 0;
        while let Some(current) = url.take() {
            let (page, next): (Vec<T>, _) = self.get_url(&current).await?;
            out.extend(page);
            pages += 1;
            if pages < max_pages {
                url = next;
            }
        }
        Ok(out)
    }

    async fn send<B: Serialize, T: DeserializeOwned>(&self, method: Method, path: &str, body: &B) -> Result<T> {
        let response = self.request(method, &format!("{}{path}", self.api_base)).json(body).send().await?;
        Ok(Self::check(response).await?.json().await?)
    }

    pub async fn user(&self) -> Result<Account> {
        self.get("/user").await
    }

    pub async fn emails(&self) -> Result<Vec<Email>> {
        self.get("/user/emails").await
    }

    pub async fn repos(&self) -> Result<Vec<Repo>> {
        self.get_all("/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member", 20)
            .await
    }

    pub async fn orgs(&self) -> Result<Vec<Org>> {
        self.get_all("/user/orgs?per_page=100", 5).await
    }

    pub async fn branch_protected(&self, owner: &str, name: &str, branch: &str) -> Result<bool> {
        #[derive(Deserialize)]
        struct Branch {
            protected: bool,
        }
        match self.get::<Branch>(&format!("/repos/{owner}/{name}/branches/{branch}")).await {
            Ok(b) => Ok(b.protected),
            Err(Error::Api { status: 404, .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn repo(&self, owner: &str, name: &str) -> Result<Repo> {
        self.get(&format!("/repos/{owner}/{name}")).await
    }

    pub async fn create_repo(&self, org: Option<&str>, name: &str, description: &str, private: bool) -> Result<Repo> {
        let body = serde_json::json!({ "name": name, "description": description, "private": private });
        let path = match org {
            Some(org) => format!("/orgs/{org}/repos"),
            None => "/user/repos".to_string(),
        };
        self.send(Method::POST, &path, &body).await
    }

    pub async fn pulls(&self, owner: &str, name: &str) -> Result<Vec<PullRequest>> {
        self.get_all(&format!("/repos/{owner}/{name}/pulls?state=open&per_page=100"), 5).await
    }

    pub async fn checks(&self, owner: &str, name: &str, git_ref: &str) -> Result<ChecksSummary> {
        let runs: CheckRuns =
            self.get(&format!("/repos/{owner}/{name}/commits/{git_ref}/check-runs?per_page=100")).await?;
        let status: CombinedStatus = self.get(&format!("/repos/{owner}/{name}/commits/{git_ref}/status")).await?;
        Ok(summarize_checks(runs.check_runs, status.statuses))
    }

    pub async fn rerun_check_suite(&self, owner: &str, name: &str, suite: u64) -> Result<()> {
        let response = self
            .request(Method::POST, &format!("{}/repos/{owner}/{name}/check-suites/{suite}/rerequest", self.api_base))
            .send()
            .await?;
        Self::check(response).await.map(drop)
    }

    pub async fn license_text(&self, key: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct License {
            body: String,
        }
        Ok(self.get::<License>(&format!("/licenses/{key}")).await?.body)
    }

    pub async fn gitignore_templates(&self) -> Result<Vec<String>> {
        self.get("/gitignore/templates").await
    }

    pub async fn gitignore_template(&self, name: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Template {
            source: String,
        }
        Ok(self.get::<Template>(&format!("/gitignore/templates/{name}")).await?.source)
    }
}

fn next_link(header: &str) -> Option<String> {
    header.split(',').find_map(|part| {
        let (url, rel) = part.split_once(';')?;
        rel.contains("rel=\"next\"").then(|| url.trim().trim_start_matches('<').trim_end_matches('>').to_string())
    })
}

fn sso_url(header: &str) -> Option<String> {
    let mut parts = header.split(';').map(str::trim);
    if !parts.next()?.eq_ignore_ascii_case("required") {
        return None;
    }
    let url = parts.find_map(|p| p.strip_prefix("url="))?;
    url.starts_with("https://").then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sso_header() {
        assert_eq!(
            sso_url("required; url=https://github.com/orgs/acme/sso?authorization_request=A1B2").as_deref(),
            Some("https://github.com/orgs/acme/sso?authorization_request=A1B2")
        );
        assert_eq!(sso_url("partial-results; organizations=21955855,20582480"), None);
        assert_eq!(sso_url("required"), None);
        assert_eq!(sso_url("required; url=http://github.com/orgs/acme/sso"), None);
    }

    #[test]
    fn parses_remotes() {
        let expect = Some(("github.com".to_string(), "acme".to_string(), "app".to_string()));
        assert_eq!(parse_remote("https://github.com/acme/app.git"), expect);
        assert_eq!(parse_remote("git@github.com:acme/app.git"), expect);
        assert_eq!(parse_remote("ssh://git@github.com/acme/app"), expect);
        assert_eq!(parse_remote("not a url"), None);
    }

    #[test]
    fn link_header_next() {
        let h = r#"<https://api.github.com/user/repos?page=2>; rel="next", <https://api.github.com/user/repos?page=5>; rel="last""#;
        assert_eq!(next_link(h).as_deref(), Some("https://api.github.com/user/repos?page=2"));
        assert_eq!(next_link(r#"<x>; rel="prev""#), None);
    }

    fn run(status: &str, conclusion: Option<&str>) -> CheckRun {
        CheckRun {
            id: 1,
            name: "ci".into(),
            status: status.into(),
            conclusion: conclusion.map(Into::into),
            html_url: None,
            check_suite: None,
        }
    }

    #[test]
    fn check_summary_states() {
        assert_eq!(summarize_checks(vec![], vec![]).state, CheckState::None);
        assert_eq!(summarize_checks(vec![run("completed", Some("success"))], vec![]).state, CheckState::Success);
        assert_eq!(summarize_checks(vec![run("in_progress", None)], vec![]).state, CheckState::Pending);
        let s = summarize_checks(vec![run("completed", Some("failure")), run("in_progress", None)], vec![]);
        assert_eq!((s.state, s.failed, s.pending), (CheckState::Failure, 1, 1));
    }
}
