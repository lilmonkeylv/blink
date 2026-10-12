use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::oauth::{OAuthApp, Token, TokenResponse};

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePoll {
    Pending,
    SlowDown(u64),
    Complete(Token),
}

impl OAuthApp {
    pub async fn start_device_flow(&self) -> Result<DeviceCode> {
        Ok(self
            .http
            .post(format!("{}/login/device/code", self.web_base))
            .header("Accept", "application/json")
            .form(&[("client_id", self.client_id.as_str()), ("scope", self.scopes.as_str())])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn poll_device_flow(&self, code: &DeviceCode) -> Result<DevicePoll> {
        let response: TokenResponse = self
            .http
            .post(format!("{}/login/oauth/access_token", self.web_base))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("device_code", code.device_code.as_str()),
                ("grant_type", DEVICE_GRANT),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        match response.error.as_deref() {
            Some("authorization_pending") => Ok(DevicePoll::Pending),
            Some("slow_down") => Ok(DevicePoll::SlowDown(response.interval.unwrap_or(code.interval + 5))),
            Some("expired_token") => Err(Error::Timeout),
            Some("access_denied") => Err(Error::Denied),
            _ => response.into_token().map(DevicePoll::Complete),
        }
    }

    pub async fn wait_for_device_token(&self, code: &DeviceCode) -> Result<Token> {
        let mut interval = code.interval.max(1);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(code.expires_in);
        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            match self.poll_device_flow(code).await? {
                DevicePoll::Pending => {}
                DevicePoll::SlowDown(next) => interval = next,
                DevicePoll::Complete(token) => return Ok(token),
            }
        }
    }
}
