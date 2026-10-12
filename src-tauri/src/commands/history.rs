use blink_core::ops;
use blink_diff::{DiffOptions, FileDiff};
use blink_git::{CommitDetail, CommitSummary};
use tauri::State;

use crate::error::CmdResult;
use crate::state::AppState;

const MAX_PAGE: usize = 500;

#[tauri::command]
pub async fn history(
    state: State<'_, AppState>,
    id: String,
    offset: usize,
    limit: usize,
    rev: Option<String>,
) -> CmdResult<Vec<CommitSummary>> {
    let limit = limit.min(MAX_PAGE);
    Ok(state.engine.read(&id, move |repo| repo.log(rev.as_deref(), offset, limit)).await?)
}

#[tauri::command]
pub async fn commit_detail(state: State<'_, AppState>, id: String, sha: String) -> CmdResult<CommitDetail> {
    Ok(state.engine.read(&id, move |repo| repo.commit_detail(&sha)).await?)
}

#[tauri::command]
pub async fn commit_file_diff(
    state: State<'_, AppState>,
    id: String,
    sha: String,
    path: String,
    old_path: Option<String>,
    ignore_whitespace: Option<bool>,
) -> CmdResult<FileDiff> {
    let opts = DiffOptions { ignore_whitespace: ignore_whitespace.unwrap_or(false), ..Default::default() };
    Ok(state.engine.read(&id, move |repo| ops::commit_diff(repo, &sha, &path, old_path.as_deref(), opts)).await?)
}
