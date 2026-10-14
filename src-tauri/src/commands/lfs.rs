use blink_git::{LfsLock, SyncAction, safe_rel_path};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::commands::sync::progress_emitter;
use crate::error::CmdResult;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsStatus {
    version: Option<String>,
    uses_lfs: bool,
}

#[tauri::command]
pub async fn lfs_status(state: State<'_, AppState>, id: String) -> CmdResult<LfsStatus> {
    Ok(state
        .engine
        .read(&id, |repo| {
            let attributes = std::fs::read_to_string(repo.workdir().join(".gitattributes")).unwrap_or_default();
            Ok(LfsStatus { version: repo.cli().lfs_version(), uses_lfs: blink_git::uses_lfs(&attributes) })
        })
        .await?)
}

#[tauri::command]
pub async fn lfs_track(state: State<'_, AppState>, id: String, pattern: String) -> CmdResult<()> {
    blink_git::validate_lfs_pattern(&pattern)?;
    Ok(state.engine.write(&id, move |repo| repo.cli().lfs_track(&pattern)).await?)
}

#[tauri::command]
pub async fn lfs_pull(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    let emit = progress_emitter(&app, &id, SyncAction::Fetch);
    Ok(state.engine.network_write(&id, move |_| cli.lfs_pull(emit)).await?)
}

#[tauri::command]
pub async fn lfs_locks(state: State<'_, AppState>, id: String) -> CmdResult<Vec<LfsLock>> {
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo()).with_ssh_batch_mode();
    Ok(state.engine.fetch_detached(&handle, move |_| cli.lfs_locks()).await?)
}

#[tauri::command]
pub async fn lfs_lock(state: State<'_, AppState>, id: String, path: String) -> CmdResult<()> {
    safe_rel_path(&path)?;
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    Ok(state.engine.network_write(&id, move |_| cli.lfs_lock(&path)).await?)
}

#[tauri::command]
pub async fn lfs_unlock(state: State<'_, AppState>, id: String, path: String, force: Option<bool>) -> CmdResult<()> {
    safe_rel_path(&path)?;
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    let force = force.unwrap_or(false);
    Ok(state.engine.network_write(&id, move |_| cli.lfs_unlock(&path, force)).await?)
}
