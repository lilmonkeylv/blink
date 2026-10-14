use blink_git::{Submodule, SyncAction, safe_rel_path};
use blink_store::RepoRecord;
use tauri::{AppHandle, State};

use crate::commands::repo::add_repository;
use crate::commands::sync::progress_emitter;
use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

#[tauri::command]
pub async fn submodules(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Submodule>> {
    Ok(state.engine.read(&id, |repo| repo.cli().submodules()).await?)
}

#[tauri::command]
pub async fn submodule_init(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, |repo| repo.cli().submodule_init()).await?)
}

#[tauri::command]
pub async fn submodule_sync(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, |repo| repo.cli().submodule_sync()).await?)
}

#[tauri::command]
pub async fn submodule_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    paths: Option<Vec<String>>,
) -> CmdResult<()> {
    let paths = paths.unwrap_or_default();
    for path in &paths {
        safe_rel_path(path)?;
    }
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    let emit = progress_emitter(&app, &id, SyncAction::Fetch);
    state
        .engine
        .network_write(&id, move |_| {
            let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
            cli.submodule_update(&paths, emit)
        })
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn open_submodule(state: State<'_, AppState>, id: String, path: String) -> CmdResult<RepoRecord> {
    let rel = safe_rel_path(&path)?.to_path_buf();
    let wanted = path.clone();
    let known =
        state.engine.read(&id, move |repo| Ok(repo.cli().submodules()?.into_iter().find(|s| s.path == wanted))).await?;
    match known {
        Some(submodule) if submodule.initialized => {}
        Some(_) => {
            return Err(CommandError::new("uninitialized", format!("{path} isn't initialized yet. Update it first.")));
        }
        None => return Err(CommandError::new("invalidArgument", format!("{path} is not a submodule"))),
    }
    let dir = state.engine.get(&id)?.repo().workdir().join(rel);
    add_repository(state, dir.to_string_lossy().into_owned()).await
}
