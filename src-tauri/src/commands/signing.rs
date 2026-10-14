use std::path::PathBuf;

use blink_git::{ConfigScope, GitCli, SigningConfig, SigningFormat, SigningKey, SshPublicKey};
use blink_github::Client;
use serde::Serialize;
use tauri::State;

use crate::error::{CmdResult, CommandError};
use crate::state::{AppState, GITHUB_HOST};

pub const GITHUB_KEYS_URL: &str = "https://github.com/settings/keys";
const KEY_TITLE: &str = "Blink signing key";

fn ssh_dir() -> CmdResult<PathBuf> {
    std::env::home_dir()
        .map(|home| home.join(".ssh"))
        .ok_or_else(|| CommandError::new("noHome", "Could not find your home folder"))
}

fn global_cli(state: &AppState) -> GitCli {
    GitCli::new(&state.paths.data_dir)
}

fn gpg_for(cli: &GitCli) -> CmdResult<blink_git::GpgProgram> {
    blink_git::find_gpg(cli.config_get("gpg.program").as_deref())
        .ok_or_else(|| CommandError::new("gpgNotFound", "GnuPG is not installed"))
}

async fn blocking<T: Send + 'static>(task: impl FnOnce() -> blink_git::Result<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| CommandError::new("core", e.to_string()))?
        .map_err(Into::into)
}

fn ssh_public_key(key: &str) -> CmdResult<(PathBuf, SshPublicKey)> {
    Ok(blink_git::ssh_key_in(&ssh_dir()?, key)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SigningStatus {
    #[serde(flatten)]
    config: SigningConfig,
    gpg_found: bool,
    ssh_keygen_found: bool,
}

#[tauri::command]
pub async fn signing_status(state: State<'_, AppState>, id: Option<String>) -> CmdResult<SigningStatus> {
    let cli = match id {
        Some(id) => state.engine.get(&id)?.repo().cli(),
        None => global_cli(&state),
    };
    blocking(move || {
        Ok(SigningStatus {
            gpg_found: blink_git::find_gpg(cli.config_get("gpg.program").as_deref()).is_some(),
            ssh_keygen_found: blink_git::find_ssh_keygen().is_some(),
            config: cli.signing_config(),
        })
    })
    .await
}

#[tauri::command]
pub async fn signing_keys(state: State<'_, AppState>, format: SigningFormat) -> CmdResult<Vec<SigningKey>> {
    match format {
        SigningFormat::Ssh => {
            let dir = ssh_dir()?;
            blocking(move || Ok(blink_git::list_ssh_keys(&dir))).await
        }
        SigningFormat::Gpg => {
            let gpg = gpg_for(&global_cli(&state))?;
            blocking(move || blink_git::list_gpg_keys(&gpg.path)).await
        }
    }
}

#[tauri::command]
pub async fn generate_ssh_signing_key(name: String, comment: String) -> CmdResult<SigningKey> {
    let dir = ssh_dir()?;
    let keygen = blink_git::find_ssh_keygen().ok_or_else(|| {
        CommandError::new("sshKeygenNotFound", "ssh-keygen was not found. Install OpenSSH, then try again.")
    })?;
    blocking(move || {
        let public = blink_git::generate_ssh_key(&keygen, &dir, &name, &comment)?;
        let id = public.to_string_lossy();
        blink_git::list_ssh_keys(&dir)
            .into_iter()
            .find(|k| k.id == id)
            .ok_or_else(|| blink_git::Error::Git("the new key could not be read".into()))
    })
    .await
}

#[tauri::command]
pub async fn configure_signing(
    state: State<'_, AppState>,
    id: Option<String>,
    scope: ConfigScope,
    format: SigningFormat,
    key: String,
) -> CmdResult<()> {
    let key = match format {
        SigningFormat::Ssh => ssh_public_key(&key)?.0.to_string_lossy().into_owned(),
        SigningFormat::Gpg => blink_git::validate_gpg_key_id(&key)?.to_string(),
    };
    let cli = match (scope, &id) {
        (ConfigScope::Global, _) => global_cli(&state),
        (ConfigScope::Local, Some(id)) => state.engine.get(id)?.repo().cli(),
        (ConfigScope::Local, None) => return Err(CommandError::new("invalidArgument", "Open a repository first")),
    };
    let program = match format {
        SigningFormat::Gpg if cli.config_get("gpg.program").is_none() => {
            Some(gpg_for(&cli)?).filter(|gpg| !gpg.on_path).map(|gpg| gpg.path)
        }
        _ => None,
    };
    let configure = move |cli: &GitCli| cli.configure_signing(scope, format, &key, program.as_deref());
    match (scope, id) {
        (ConfigScope::Local, Some(id)) => Ok(state.engine.write(&id, move |repo| configure(&repo.cli())).await?),
        _ => blocking(move || configure(&cli)).await,
    }
}

#[tauri::command]
pub async fn signing_public_key(state: State<'_, AppState>, format: SigningFormat, key: String) -> CmdResult<String> {
    match format {
        SigningFormat::Ssh => Ok(ssh_public_key(&key)?.1.openssh()),
        SigningFormat::Gpg => {
            let gpg = gpg_for(&global_cli(&state))?;
            blocking(move || blink_git::export_gpg_public_key(&gpg.path, &key)).await
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubSigningKey {
    login: String,
    added: bool,
    verified_emails: Vec<String>,
    unverified_emails: Vec<String>,
}

async fn github_key_state(gh: &Client, login: &str, format: SigningFormat, key: &str) -> CmdResult<GithubSigningKey> {
    let mut found = GithubSigningKey { login: login.to_string(), ..Default::default() };
    match format {
        SigningFormat::Ssh => {
            let (_, public) = ssh_public_key(key)?;
            found.added =
                gh.ssh_signing_keys(login).await?.iter().any(|k| {
                    SshPublicKey::parse(&k.key).is_some_and(|k| k.kind == public.kind && k.blob == public.blob)
                });
        }
        SigningFormat::Gpg => {
            let id = blink_git::validate_gpg_key_id(key)?;
            if let Some(known) = gh.gpg_keys(login).await?.into_iter().find(|k| k.matches(id)) {
                found.added = true;
                for email in known.emails {
                    if email.verified {
                        found.verified_emails.push(email.email);
                    } else {
                        found.unverified_emails.push(email.email);
                    }
                }
            }
        }
    }
    Ok(found)
}

fn signed_in(state: &AppState) -> Option<(String, Client)> {
    let login = state.account()?.login;
    let gh = state.client_as(GITHUB_HOST, &login)?;
    Some((login, gh))
}

#[tauri::command]
pub async fn github_signing_key(
    state: State<'_, AppState>,
    format: SigningFormat,
    key: String,
) -> CmdResult<Option<GithubSigningKey>> {
    let Some((login, gh)) = signed_in(&state) else { return Ok(None) };
    github_key_state(&gh, &login, format, &key).await.map(Some)
}

fn missing_scope() -> CommandError {
    CommandError {
        kind: "missingScope",
        message: "Blink's GitHub sign-in isn't allowed to add signing keys. Copy the public key and add it on GitHub instead."
            .into(),
        detail: Some(GITHUB_KEYS_URL.into()),
    }
}

#[tauri::command]
pub async fn add_signing_key_to_github(
    state: State<'_, AppState>,
    format: SigningFormat,
    key: String,
) -> CmdResult<GithubSigningKey> {
    let (login, gh) = signed_in(&state).ok_or_else(|| CommandError::new("signedOut", "Sign in to GitHub.com first"))?;
    let added = match format {
        SigningFormat::Ssh => {
            let (_, public) = ssh_public_key(&key)?;
            gh.add_ssh_signing_key(KEY_TITLE, &public.openssh()).await.map(drop)
        }
        SigningFormat::Gpg => {
            let gpg = gpg_for(&global_cli(&state))?;
            let id = key.clone();
            let armored = blocking(move || blink_git::export_gpg_public_key(&gpg.path, &id)).await?;
            gh.add_gpg_key(KEY_TITLE, &armored).await.map(drop)
        }
    };
    match added {
        Ok(()) => {}
        Err(blink_github::Error::Api { status: 403 | 404, .. }) => return Err(missing_scope()),
        Err(blink_github::Error::Api { status: 422, message }) => {
            return Err(CommandError {
                kind: "rejected",
                message: "GitHub did not accept this key".into(),
                detail: Some(message),
            });
        }
        Err(e) => return Err(e.into()),
    }
    let mut found = github_key_state(&gh, &login, format, &key).await.unwrap_or_default();
    found.login = login;
    found.added = true;
    Ok(found)
}
