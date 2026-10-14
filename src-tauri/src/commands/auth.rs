use blink_github::{Account, Client, DeviceCode, OAuthApp, Token};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::accounts::{self, same_login};
use crate::error::{CmdResult, CommandError};
use crate::state::{AppState, EnterpriseAccount, GITHUB_HOST, api_base, normalize_host};

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

async fn finish(state: &AppState, token: &str) -> CmdResult<SignedIn> {
    let client = Client::github_com(token);
    let account = client.user().await?;
    let primary_email = client
        .emails()
        .await
        .ok()
        .and_then(|emails| emails.into_iter().find(|e| e.primary && e.verified).map(|e| e.email));
    state.secrets.set(&AppState::token_key(&account.login), token)?;
    let mut list = state.github_accounts();
    accounts::upsert(&mut list, account.clone());
    state.save_github_accounts(&list)?;
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
    let token: Token = oauth.complete(request).await?;
    focus_main(&app);
    finish(&state, &token.access_token).await
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
    finish(&state, &token.access_token).await
}

#[tauri::command]
pub async fn token_sign_in(state: State<'_, AppState>, token: String) -> CmdResult<SignedIn> {
    let token = token.trim();
    if token.is_empty() {
        return Err(CommandError::new("token", "Paste a personal access token"));
    }
    finish(&state, token).await
}

#[tauri::command(async)]
pub fn sign_out(state: State<'_, AppState>, login: Option<String>) -> CmdResult<()> {
    let mut list = state.github_accounts();
    let Some(target) = login.or_else(|| list.first().map(|a| a.login.clone())) else { return Ok(()) };
    let Some(index) = list.iter().position(|a| same_login(&a.login, &target)) else { return Ok(()) };
    let removed = list.remove(index);
    state.secrets.delete(&AppState::token_key(&removed.login))?;
    state.save_github_accounts(&list)?;
    crate::commands::github::forget_linked_emails(GITHUB_HOST);
    Ok(())
}

#[tauri::command]
pub fn github_accounts(state: State<'_, AppState>) -> Vec<Account> {
    state.github_accounts()
}

#[tauri::command]
pub fn set_primary_account(state: State<'_, AppState>, login: String) -> CmdResult<()> {
    let mut list = state.github_accounts();
    if accounts::make_primary(&mut list, &login) {
        state.save_github_accounts(&list)?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoAccount {
    account: Option<Account>,
    chosen: Option<String>,
}

#[tauri::command]
pub async fn repo_account(state: State<'_, AppState>, id: String) -> CmdResult<RepoAccount> {
    let handle = state.engine.get(&id)?;
    let account = accounts::resolve(&state, &id, handle.repo()).await;
    let list = state.github_accounts();
    let chosen = state.repo_account_choice(&id).filter(|c| list.iter().any(|a| same_login(&a.login, c)));
    Ok(RepoAccount { account, chosen })
}

#[tauri::command(async)]
pub fn set_repo_account(state: State<'_, AppState>, id: String, login: Option<String>) -> CmdResult<()> {
    Ok(state.set_repo_account_choice(&id, login.as_deref())?)
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
    state.forget_credentials(&host);
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
    state.forget_credentials(&host);
    crate::commands::github::forget_linked_emails(&host);
    Ok(())
}
