use blink_github::{Account, Client, DeviceCode, OAuthApp, Token};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CmdResult, CommandError};
use crate::state::{ACCOUNT_KEY, AppState, EnterpriseAccount, GITHUB_HOST, api_base, normalize_host};

fn oauth_app(state: &AppState) -> CmdResult<OAuthApp> {
    let client_id = state.oauth.client_id.clone().ok_or_else(|| {
        CommandError::new("oauthNotConfigured", "Set BLINK_GITHUB_CLIENT_ID to enable GitHub sign-in")
    })?;
    Ok(OAuthApp::github_com(client_id, state.oauth.client_secret.clone()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedIn {
    account: Account,
    primary_email: Option<String>,
}

async fn finish(state: &AppState, token: Token) -> CmdResult<SignedIn> {
    let client = Client::github_com(&token.access_token);
    let account = client.user().await?;
    let primary_email = client
        .emails()
        .await
        .ok()
        .and_then(|emails| emails.into_iter().find(|e| e.primary && e.verified).map(|e| e.email));
    state.secrets.set(&AppState::token_key(&account.login), &token.access_token)?;
    state.cache.put_app_state(ACCOUNT_KEY, &account)?;
    Ok(SignedIn { account, primary_email })
}

fn focus_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub async fn sign_in(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SignedIn> {
    let oauth = oauth_app(&state)?;
    let request = oauth.begin().await?;
    app.opener().open_url(&request.url, None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))?;
    let token = oauth.complete(request).await?;
    focus_main(&app);
    finish(&state, token).await
}

#[tauri::command]
pub async fn device_start(app: AppHandle, state: State<'_, AppState>) -> CmdResult<DeviceCode> {
    let code = oauth_app(&state)?.start_device_flow().await?;
    let _ = app.opener().open_url(&code.verification_uri, None::<&str>);
    Ok(code)
}

#[tauri::command]
pub async fn device_complete(app: AppHandle, state: State<'_, AppState>, code: DeviceCode) -> CmdResult<SignedIn> {
    let token = oauth_app(&state)?.wait_for_device_token(&code).await?;
    focus_main(&app);
    finish(&state, token).await
}

#[tauri::command]
pub fn sign_out(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(account) = state.account() {
        state.secrets.delete(&AppState::token_key(&account.login))?;
    }
    state.cache.put_app_state(ACCOUNT_KEY, &Option::<Account>::None)?;
    *state.github.lock() = None;
    Ok(())
}

#[tauri::command]
pub fn revoke_url(state: State<'_, AppState>) -> Option<String> {
    state.oauth.client_id.as_ref().map(|id| format!("https://github.com/settings/connections/applications/{id}"))
}

#[tauri::command]
pub fn enterprise_accounts(state: State<'_, AppState>) -> Vec<EnterpriseAccount> {
    state.enterprise_accounts()
}

#[tauri::command]
pub async fn enterprise_sign_in(
    state: State<'_, AppState>,
    host: String,
    token: String,
) -> CmdResult<EnterpriseAccount> {
    let host =
        normalize_host(&host).ok_or_else(|| CommandError::new("host", "That doesn't look like a server address"))?;
    if host == GITHUB_HOST {
        return Err(CommandError::new("host", "Use the GitHub.com sign-in for github.com"));
    }
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(CommandError::new("token", "Paste a personal access token"));
    }
    let account = Client::new(api_base(&host), &token).user().await?;
    state.secrets.set(&AppState::host_token_key(&host, &account.login), &token)?;
    let entry = EnterpriseAccount { host: host.clone(), account };
    let mut accounts = state.enterprise_accounts();
    if let Some(old) = accounts.iter().find(|a| a.host == host)
        && old.account.login != entry.account.login
    {
        state.secrets.delete(&AppState::host_token_key(&host, &old.account.login))?;
    }
    accounts.retain(|a| a.host != host);
    accounts.push(entry.clone());
    state.save_enterprise_accounts(&accounts)?;
    Ok(entry)
}

#[tauri::command]
pub fn enterprise_sign_out(state: State<'_, AppState>, host: String) -> CmdResult<()> {
    let mut accounts = state.enterprise_accounts();
    if let Some(old) = accounts.iter().find(|a| a.host == host) {
        state.secrets.delete(&AppState::host_token_key(&host, &old.account.login))?;
    }
    accounts.retain(|a| a.host != host);
    state.save_enterprise_accounts(&accounts)?;
    Ok(())
}
