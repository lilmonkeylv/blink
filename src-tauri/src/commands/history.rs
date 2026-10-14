use blink_core::ops;
use blink_diff::DiffOptions;
use blink_git::{CommitDetail, safe_rel_path, validate_sha};
use tauri::State;
use tauri::ipc::Response;

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
) -> CmdResult<Response> {
    let limit = limit.min(MAX_PAGE);
    let commits = state.engine.read(&id, move |repo| repo.log(rev.as_deref(), offset, limit)).await?;
    Ok(Response::new(crate::wire::encode_history(&commits)))
}

#[tauri::command]
pub async fn commit_detail(state: State<'_, AppState>, id: String, sha: String) -> CmdResult<CommitDetail> {
    validate_sha(&sha)?;
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
    highlight: Option<bool>,
) -> CmdResult<Response> {
    validate_sha(&sha)?;
    safe_rel_path(&path)?;
    let opts = DiffOptions {
        ignore_whitespace: ignore_whitespace.unwrap_or(false),
        highlight: highlight.unwrap_or(true),
        ..Default::default()
    };
    let diff =
        state.engine.read(&id, move |repo| ops::commit_diff(repo, &sha, &path, old_path.as_deref(), opts)).await?;
    Ok(crate::wire::diff_response(&diff))
}
