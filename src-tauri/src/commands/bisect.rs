use blink_git::{BisectMark, BisectState};
use tauri::State;

use crate::error::CmdResult;
use crate::state::AppState;

async fn current_state(state: &AppState, id: &str) -> CmdResult<Option<BisectState>> {
    Ok(state.engine.read(id, |repo| repo.cli().bisect_state(repo.git_dir())).await?)
}

#[tauri::command]
pub async fn bisect_state(state: State<'_, AppState>, id: String) -> CmdResult<Option<BisectState>> {
    current_state(&state, &id).await
}

#[tauri::command]
pub async fn bisect_start(
    state: State<'_, AppState>,
    id: String,
    bad: String,
    good: Option<String>,
) -> CmdResult<Option<BisectState>> {
    state.engine.write(&id, move |repo| repo.cli().bisect_start(&bad, good.as_deref())).await?;
    current_state(&state, &id).await
}

#[tauri::command]
pub async fn bisect_mark(
    state: State<'_, AppState>,
    id: String,
    mark: BisectMark,
    sha: Option<String>,
) -> CmdResult<Option<BisectState>> {
    state.engine.write(&id, move |repo| repo.cli().bisect_mark(mark, sha.as_deref())).await?;
    current_state(&state, &id).await
}

#[tauri::command]
pub async fn bisect_reset(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, |repo| repo.cli().bisect_reset()).await?)
}
