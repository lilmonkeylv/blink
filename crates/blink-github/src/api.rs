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
pub struct Verification {
    pub verified: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshSigningKey {
    pub id: u64,
    pub key: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpgKeyEmail {
    pub email: String,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpgSubkey {
    pub key_id: String,
    #[serde(default)]
    pub can_sign: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpgKey {
    pub id: u64,
    pub key_id: String,
    #[serde(default)]
    pub can_sign: bool,
    #[serde(default)]
    pub emails: Vec<GpgKeyEmail>,
    #[serde(default)]
    pub subkeys: Vec<GpgSubkey>,
}

impl GpgKey {
    pub fn matches(&self, key_id: &str) -> bool {
        let wanted = key_id.trim_start_matches("0x");
        let same = |id: &str| {
            let n = id.len().min(wanted.len());
            let tail = |s: &str| s.get(s.len() - n..).map(str::to_ascii_uppercase);
            n >= 8 && tail(id).is_some() && tail(id) == tail(wanted)
        };
        same(&self.key_id) || self.subkeys.iter().any(|s| same(&s.key_id))
    }
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
    #[serde(default)]
    pub permissions: Option<Permissions>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Permissions {
    #[serde(default)]
    pub push: bool,
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
    #[serde(default)]
    pub checks: Option<CheckState>,
}

const PULLS_QUERY: &str = "query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    pullRequests(states: OPEN, first: 100, orderBy: {field: CREATED_AT, direction: DESC}) {
      nodes {
        number title url isDraft createdAt updatedAt
        author { login }
        headRefName headRefOid
        headRepository { nameWithOwner url }
        baseRefName baseRefOid
        commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }
      }
    }
  }
}";

#[derive(Deserialize)]
struct PullsData {
    repository: Option<GqlRepository>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlRepository {
    pull_requests: GqlNodes<GqlPull>,
}

#[derive(Deserialize)]
struct GqlNodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
struct GqlLogin {
    login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlHeadRepository {
    name_with_owner: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlPull {
    number: u64,
    title: String,
    url: String,
    is_draft: bool,
    created_at: String,
    updated_at: String,
    author: Option<GqlLogin>,
    head_ref_name: String,
    head_ref_oid: String,
    head_repository: Option<GqlHeadRepository>,
    base_ref_name: String,
    base_ref_oid: String,
    commits: GqlNodes<GqlPullCommit>,
}

#[derive(Deserialize)]
struct GqlPullCommit {
    commit: GqlCommit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlCommit {
    status_check_rollup: Option<GqlRollup>,
}

#[derive(Deserialize)]
struct GqlRollup {
    state: String,
}

impl From<GqlPull> for PullRequest {
    fn from(pr: GqlPull) -> Self {
        let checks = match pr.commits.nodes.last().and_then(|c| c.commit.status_check_rollup.as_ref()) {
            None => CheckState::None,
            Some(rollup) => match rollup.state.as_str() {
                "SUCCESS" => CheckState::Success,
                "FAILURE" | "ERROR" => CheckState::Failure,
                _ => CheckState::Pending,
            },
        };
        PullRequest {
            number: pr.number,
            title: pr.title,
            html_url: pr.url,
            state: "open".to_string(),
            draft: pr.is_draft,
            user: Owner { login: pr.author.map_or_else(|| "ghost".to_string(), |a| a.login) },
            head: PullRef {
                name: pr.head_ref_name,
                sha: pr.head_ref_oid,
                repo: pr
                    .head_repository
                    .map(|r| PullRepo { full_name: r.name_with_owner, clone_url: format!("{}.git", r.url) }),
            },
            base: PullRef { name: pr.base_ref_name, sha: pr.base_ref_oid, repo: None },
            created_at: pr.created_at,
            updated_at: pr.updated_at,
            checks: Some(checks),
        }
    }
}

fn pulls_from_graphql(data: PullsData) -> Vec<PullRequest> {
    data.repository.map(|r| r.pull_requests.nodes.into_iter().map(Into::into).collect()).unwrap_or_default()
}

#[derive(Deserialize)]
struct GraphqlReply<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<ApiMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckSuiteRef {
    pub id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckApp {
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: Option<String>,
    pub check_suite: Option<CheckSuiteRef>,
    #[serde(default)]
    pub app: Option<CheckApp>,
}

#[derive(Deserialize)]
struct CheckRuns {
    check_runs: Vec<CheckRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobStep {
    pub number: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub run_id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: Option<String>,
    #[serde(default)]
    pub steps: Vec<JobStep>,
}

#[derive(Deserialize)]
struct Jobs {
    jobs: Vec<Job>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        Ok(self.get_url(&format!("{}{path}", self.api_base)).await?.0)
    }

    pub(crate) async fn get_all<T: DeserializeOwned>(&self, path: &str, max_pages: usize) -> Result<Vec<T>> {
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

    pub(crate) async fn send<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<T> {
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

    pub async fn ssh_signing_keys(&self, login: &str) -> Result<Vec<SshSigningKey>> {
        self.get_all(&format!("/users/{login}/ssh_signing_keys?per_page=100"), 3).await
    }

    pub async fn gpg_keys(&self, login: &str) -> Result<Vec<GpgKey>> {
        self.get_all(&format!("/users/{login}/gpg_keys?per_page=100"), 3).await
    }

    pub async fn add_ssh_signing_key(&self, title: &str, key: &str) -> Result<SshSigningKey> {
        let body = serde_json::json!({ "title": title, "key": key });
        self.send(Method::POST, "/user/ssh_signing_keys", &body).await
    }

    pub async fn add_gpg_key(&self, name: &str, armored: &str) -> Result<GpgKey> {
        let body = serde_json::json!({ "name": name, "armored_public_key": armored });
        self.send(Method::POST, "/user/gpg_keys", &body).await
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

    pub async fn commit_verification(&self, owner: &str, name: &str, sha: &str) -> Result<Option<Verification>> {
        #[derive(Deserialize)]
        struct Inner {
            verification: Verification,
        }
        #[derive(Deserialize)]
        struct Commit {
            commit: Inner,
        }
        match self.get::<Commit>(&format!("/repos/{owner}/{name}/commits/{sha}")).await {
            Ok(c) => Ok(Some(c.commit.verification)),
            Err(Error::Api { status: 404 | 422, .. }) => Ok(None),
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

    fn graphql_url(&self) -> String {
        match self.api_base.strip_suffix("/api/v3") {
            Some(web) => format!("{web}/api/graphql"),
            None => format!("{}/graphql", self.api_base),
        }
    }

    pub async fn graphql<T: DeserializeOwned>(&self, query: &str, variables: serde_json::Value) -> Result<T> {
        let body = serde_json::json!({ "query": query, "variables": variables });
        let response = self.request(Method::POST, &self.graphql_url()).json(&body).send().await?;
        let reply: GraphqlReply<T> = Self::check(response).await?.json().await?;
        if let Some(error) = reply.errors.into_iter().next() {
            return Err(Error::Graphql(error.message));
        }
        reply.data.ok_or_else(|| Error::Graphql("response has no data".to_string()))
    }

    pub async fn pulls(&self, owner: &str, name: &str) -> Result<Vec<PullRequest>> {
        let variables = serde_json::json!({ "owner": owner, "name": name });
        match self.graphql::<PullsData>(PULLS_QUERY, variables).await {
            Ok(data) => Ok(pulls_from_graphql(data)),
            Err(Error::Unauthorized) => Err(Error::Unauthorized),
            Err(e) => {
                tracing::debug!(error = %e, "GraphQL pull request query failed, falling back to REST");
                self.get_all(&format!("/repos/{owner}/{name}/pulls?state=open&per_page=100"), 5).await
            }
        }
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

    pub async fn run_jobs(&self, owner: &str, name: &str, run_id: u64) -> Result<Vec<Job>> {
        Ok(self.get::<Jobs>(&format!("/repos/{owner}/{name}/actions/runs/{run_id}/jobs?per_page=100")).await?.jobs)
    }

    pub async fn job(&self, owner: &str, name: &str, job_id: u64) -> Result<Job> {
        self.get(&format!("/repos/{owner}/{name}/actions/jobs/{job_id}")).await
    }

    pub async fn job_log(&self, owner: &str, name: &str, job_id: u64) -> Result<String> {
        let url = format!("{}/repos/{owner}/{name}/actions/jobs/{job_id}/logs", self.api_base);
        let response = self.request(Method::GET, &url).send().await?;
        Ok(Self::check(response).await?.text().await?)
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
    fn matches_gpg_keys_by_long_or_short_id() {
        let key: GpgKey = serde_json::from_str(
            r#"{"id": 3, "key_id": "3AA5C34371567BD2", "can_sign": true,
                "emails": [{"email": "mona@example.com", "verified": true}],
                "subkeys": [{"key_id": "4A595D4C72EE49C7", "can_sign": false}]}"#,
        )
        .unwrap();
        assert!(key.matches("3aa5c34371567bd2"));
        assert!(key.matches("71567BD2"));
        assert!(key.matches("0x4A595D4C72EE49C7"));
        assert!(!key.matches("1111111111111111"));
        assert!(!key.matches("BD2"));
        assert!(key.emails[0].verified);
    }

    #[test]
    fn link_header_next() {
        let h = r#"<https://api.github.com/user/repos?page=2>; rel="next", <https://api.github.com/user/repos?page=5>; rel="last""#;
        assert_eq!(next_link(h).as_deref(), Some("https://api.github.com/user/repos?page=2"));
        assert_eq!(next_link(r#"<x>; rel="prev""#), None);
    }

    #[test]
    fn graphql_endpoints() {
        assert_eq!(Client::github_com("t").graphql_url(), "https://api.github.com/graphql");
        assert_eq!(Client::new("https://api.acme.ghe.com", "t").graphql_url(), "https://api.acme.ghe.com/graphql");
        assert_eq!(Client::new("https://ghe.acme.com/api/v3", "t").graphql_url(), "https://ghe.acme.com/api/graphql");
    }

    #[test]
    fn maps_graphql_pull_requests() {
        let json = r#"{
          "data": {
            "repository": {
              "pullRequests": {
                "nodes": [
                  {
                    "number": 42,
                    "title": "Add commit graph lanes",
                    "url": "https://github.com/acme/app/pull/42",
                    "isDraft": false,
                    "createdAt": "2026-10-01T10:00:00Z",
                    "updatedAt": "2026-10-02T11:00:00Z",
                    "author": { "login": "octocat" },
                    "headRefName": "feature/graph",
                    "headRefOid": "abc123",
                    "headRepository": { "nameWithOwner": "octocat/app", "url": "https://github.com/octocat/app" },
                    "baseRefName": "main",
                    "baseRefOid": "def456",
                    "commits": { "nodes": [{ "commit": { "statusCheckRollup": { "state": "FAILURE" } } }] }
                  },
                  {
                    "number": 7,
                    "title": "Draft from a deleted fork",
                    "url": "https://github.com/acme/app/pull/7",
                    "isDraft": true,
                    "createdAt": "2026-09-01T10:00:00Z",
                    "updatedAt": "2026-09-01T10:00:00Z",
                    "author": null,
                    "headRefName": "wip",
                    "headRefOid": "111111",
                    "headRepository": null,
                    "baseRefName": "main",
                    "baseRefOid": "def456",
                    "commits": { "nodes": [{ "commit": { "statusCheckRollup": null } }] }
                  }
                ]
              }
            }
          }
        }"#;
        let reply: GraphqlReply<PullsData> = serde_json::from_str(json).unwrap();
        assert!(reply.errors.is_empty());
        let pulls = pulls_from_graphql(reply.data.unwrap());
        assert_eq!(pulls.len(), 2);
        let first = &pulls[0];
        assert_eq!((first.number, first.state.as_str(), first.draft), (42, "open", false));
        assert_eq!(first.html_url, "https://github.com/acme/app/pull/42");
        assert_eq!(first.user.login, "octocat");
        assert_eq!((first.head.name.as_str(), first.head.sha.as_str()), ("feature/graph", "abc123"));
        assert_eq!(
            first.head.repo,
            Some(PullRepo { full_name: "octocat/app".into(), clone_url: "https://github.com/octocat/app.git".into() })
        );
        assert_eq!((first.base.name.as_str(), first.base.sha.as_str()), ("main", "def456"));
        assert_eq!(first.checks, Some(CheckState::Failure));
        let second = &pulls[1];
        assert_eq!((second.user.login.as_str(), second.draft), ("ghost", true));
        assert_eq!(second.head.repo, None);
        assert_eq!(second.checks, Some(CheckState::None));
    }

    #[test]
    fn graphql_errors_are_reported() {
        let json = r#"{ "data": null, "errors": [{ "message": "Could not resolve to a Repository" }] }"#;
        let reply: GraphqlReply<PullsData> = serde_json::from_str(json).unwrap();
        assert_eq!(reply.errors[0].message, "Could not resolve to a Repository");
        assert!(reply.data.is_none());
    }

    fn run(status: &str, conclusion: Option<&str>) -> CheckRun {
        CheckRun {
            id: 1,
            name: "ci".into(),
            status: status.into(),
            conclusion: conclusion.map(Into::into),
            html_url: None,
            check_suite: None,
            app: None,
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
