use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::loopback::{DEFAULT_TIMEOUT, LoopbackServer};
use crate::pkce::Pkce;

pub const DEFAULT_SCOPES: &str = "repo user workflow";

#[derive(Debug, Clone)]
pub struct OAuthApp {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub web_base: String,
    pub api_base: String,
    pub scopes: String,
    pub(crate) http: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub token_type: String,
}

pub struct AuthorizationRequest {
    pub url: String,
    pkce: Pkce,
    server: LoopbackServer,
}

#[derive(Deserialize)]
pub(crate) struct TokenResponse {
    access_token: Option<String>,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    token_type: String,
    pub(crate) error: Option<String>,
    error_description: Option<String>,
    pub(crate) interval: Option<u64>,
}

impl TokenResponse {
    pub(crate) fn into_token(self) -> Result<Token> {
        match self.access_token {
            Some(access_token) => Ok(Token { access_token, scope: self.scope, token_type: self.token_type }),
            None => Err(Error::OAuth {
                error: self.error.unwrap_or_else(|| "unknown_error".into()),
                description: self.error_description,
            }),
        }
    }
}

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("Blink/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .expect("http client")
}

impl OAuthApp {
    pub fn github_com(client_id: impl Into<String>, client_secret: Option<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret,
            web_base: "https://github.com".into(),
            api_base: "https://api.github.com".into(),
            scopes: DEFAULT_SCOPES.into(),
            http: http_client(),
        }
    }

    pub fn enterprise(host: &str, client_id: impl Into<String>, client_secret: Option<String>) -> Self {
        let host = host.trim_end_matches('/');
        let web = if host.starts_with("http") { host.to_string() } else { format!("https://{host}") };
        Self { api_base: format!("{web}/api/v3"), web_base: web, ..Self::github_com(client_id, client_secret) }
    }

    pub async fn begin(&self) -> Result<AuthorizationRequest> {
        let pkce = Pkce::generate();
        let server = LoopbackServer::bind().await?;
        let mut url = url::Url::parse(&format!("{}/login/oauth/authorize", self.web_base)).expect("valid base");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &server.redirect_uri())
            .append_pair("scope", &self.scopes)
            .append_pair("state", &pkce.state)
            .append_pair("code_challenge", &pkce.challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("allow_signup", "true");
        Ok(AuthorizationRequest { url: url.into(), pkce, server })
    }

    pub async fn complete(&self, request: AuthorizationRequest) -> Result<Token> {
        let redirect = request.server.redirect_uri();
        let code = request.server.wait_for_code(&request.pkce.state, DEFAULT_TIMEOUT).await?;
        self.exchange_code(&code, &request.pkce.verifier, &redirect).await
    }

    pub async fn exchange_code(&self, code: &str, verifier: &str, redirect_uri: &str) -> Result<Token> {
        let mut form = vec![
            ("client_id", self.client_id.as_str()),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ];
        if let Some(secret) = &self.client_secret {
            form.push(("client_secret", secret));
        }
        let response: TokenResponse = self
            .http
            .post(format!("{}/login/oauth/access_token", self.web_base))
            .header("Accept", "application/json")
            .form(&form)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        response.into_token()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn authorize_url_carries_pkce_and_loopback() {
        let app = OAuthApp::github_com("cid", None);
        let req = app.begin().await.unwrap();
        let url = url::Url::parse(&req.url).unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("github.com"));
        assert_eq!(q["client_id"], "cid");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], req.pkce.challenge);
        assert_eq!(q["state"], req.pkce.state);
        assert!(q["redirect_uri"].starts_with("http://127.0.0.1:"));
    }

    #[test]
    fn enterprise_bases() {
        let app = OAuthApp::enterprise("ghe.acme.com/", "cid", None);
        assert_eq!(app.web_base, "https://ghe.acme.com");
        assert_eq!(app.api_base, "https://ghe.acme.com/api/v3");
    }
}
