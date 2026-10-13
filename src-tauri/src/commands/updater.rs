use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::{CmdResult, CommandError};

pub const UPDATE_AVAILABLE: &str = "update-available";
pub const UPDATE_PROGRESS: &str = "update-progress";

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

fn updater_error(e: impl std::fmt::Display) -> CommandError {
    CommandError::new("updater", e.to_string())
}

async fn check(app: &AppHandle) -> CmdResult<Option<UpdateInfo>> {
    if cfg!(debug_assertions) && std::env::var_os("BLINK_UPDATER_DEV").is_none() {
        return Ok(None);
    }
    let update = app.updater().map_err(updater_error)?.check().await.map_err(updater_error)?;
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
