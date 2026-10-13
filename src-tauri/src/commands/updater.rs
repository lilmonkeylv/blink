use std::time::Duration;

use blink_store::UpdateChannel;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

pub const UPDATE_AVAILABLE: &str = "update-available";
pub const UPDATE_PROGRESS: &str = "update-progress";

// Prereleases never become GitHub's "latest" release, so beta.json lives on a rolling release.
const BETA_ENDPOINT: &str = "https://github.com/lilmonkeylv/blink/releases/download/updater/beta.json";

const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    downloaded: u64,
    total: Option<u64>,
}

fn channel_for(setting: UpdateChannel, running_prerelease: bool) -> UpdateChannel {
    if running_prerelease { UpdateChannel::Beta } else { setting }
}

fn updater_error(e: impl std::fmt::Display) -> CommandError {
    CommandError::new("updater", e.to_string())
}

async fn check(app: &AppHandle) -> CmdResult<Option<UpdateInfo>> {
    if cfg!(debug_assertions) && std::env::var_os("BLINK_UPDATER_DEV").is_none() {
        return Ok(None);
    }
    if std::env::var_os("FLATPAK_ID").is_some() {
        return Ok(None);
    }
    let setting = app.state::<AppState>().settings.read().advanced.update_channel;
    let updater = match channel_for(setting, !app.package_info().version.pre.is_empty()) {
        UpdateChannel::Stable => app.updater(),
        UpdateChannel::Beta => {
            let endpoint = Url::parse(BETA_ENDPOINT).map_err(updater_error)?;
            app.updater_builder().endpoints(vec![endpoint]).and_then(|b| b.build())
        }
    };
    let update = match updater.map_err(updater_error)?.check().await {
        Ok(update) => update,
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => None,
        Err(e) => return Err(updater_error(e)),
    };
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
    });
    *app.state::<PendingUpdate>().0.lock() = update;
    Ok(info)
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> CmdResult<Option<UpdateInfo>> {
    check(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle, pending: State<'_, PendingUpdate>) -> CmdResult<()> {
    let update = pending.0.lock().take();
    let update = match update {
        Some(update) => update,
        None => {
            check(&app).await?;
            app.state::<PendingUpdate>()
                .0
                .lock()
                .take()
                .ok_or_else(|| CommandError::new("noUpdate", "Blink is up to date"))?
        }
    };
    let mut downloaded = 0u64;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit(UPDATE_PROGRESS, ProgressEvent { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(updater_error)?;
    app.restart()
}

pub fn spawn_update_checks(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            match check(&app).await {
                Ok(Some(info)) => {
                    let _ = app.emit(UPDATE_AVAILABLE, info);
                }
                Ok(None) => {}
                Err(e) => tracing::debug!(error = %e.message, "update check failed"),
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prerelease_builds_follow_beta() {
        assert_eq!(channel_for(UpdateChannel::Stable, true), UpdateChannel::Beta);
        assert_eq!(channel_for(UpdateChannel::Beta, true), UpdateChannel::Beta);
        assert_eq!(channel_for(UpdateChannel::Stable, false), UpdateChannel::Stable);
        assert_eq!(channel_for(UpdateChannel::Beta, false), UpdateChannel::Beta);
    }
}
