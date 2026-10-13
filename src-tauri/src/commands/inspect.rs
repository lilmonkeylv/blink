use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use blink_diff::{DiffOptions, diff_file};
use blink_git::{Blame, BlobSource, CommitFile, CommitSummary, ReflogEntry, SearchKind, Worktree, graph_layout};
use serde::Serialize;
use tauri::State;
use tauri::ipc::Response;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

const GRAPH_LIMIT: usize = 10_000;

#[tauri::command]
pub async fn blame(state: State<'_, AppState>, id: String, path: String, rev: Option<String>) -> CmdResult<Blame> {
    Ok(state.engine.read(&id, move |repo| repo.cli().blame(&path, rev.as_deref())).await?)
}

#[tauri::command]
pub async fn file_history(
    state: State<'_, AppState>,
    id: String,
    path: String,
    offset: usize,
    limit: usize,
) -> CmdResult<Vec<CommitSummary>> {
    Ok(state.engine.read(&id, move |repo| repo.cli().file_history(&path, offset, limit.min(500))).await?)
}

#[tauri::command]
pub async fn search_history(
    state: State<'_, AppState>,
    id: String,
    kind: SearchKind,
    query: String,
) -> CmdResult<Vec<CommitSummary>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(state.engine.read(&id, move |repo| repo.cli().search(kind, query.trim(), 500)).await?)
}

#[tauri::command]
pub async fn reflog(state: State<'_, AppState>, id: String) -> CmdResult<Vec<ReflogEntry>> {
    Ok(state.engine.read(&id, |repo| repo.cli().reflog(300)).await?)
}

#[tauri::command]
pub async fn history_graph(state: State<'_, AppState>, id: String, offset: usize, limit: usize) -> CmdResult<Response> {
    let end = (offset + limit.min(500)).min(GRAPH_LIMIT);
    Ok(state
        .engine
        .read(&id, move |repo| {
            let commits = repo.log(None, 0, end)?;
            let rows = graph_layout(&commits);
            let start = offset.min(commits.len());
            Ok(Response::new(crate::wire::encode_graph(&commits[start..], &rows[start..])))
        })
        .await?)
}

#[tauri::command]
pub async fn range_files(
    state: State<'_, AppState>,
    id: String,
    oldest: String,
    newest: String,
) -> CmdResult<Vec<CommitFile>> {
    Ok(state.engine.read(&id, move |repo| repo.range_files(&oldest, &newest)).await?)
}

#[tauri::command]
pub async fn range_file_diff(
    state: State<'_, AppState>,
    id: String,
    oldest: String,
    newest: String,
    path: String,
    old_path: Option<String>,
    highlight: Option<bool>,
) -> CmdResult<Response> {
    let opts = DiffOptions { highlight: highlight.unwrap_or(true), ..Default::default() };
    let diff = state
        .engine
        .read(&id, move |repo| {
            let old =
                repo.read_blob(&BlobSource::Revision(format!("{oldest}^")), old_path.as_deref().unwrap_or(&path))?;
            let new = repo.read_blob(&BlobSource::Revision(newest), &path)?;
            Ok(diff_file(&path, &old.unwrap_or_default(), &new.unwrap_or_default(), opts))
        })
        .await?;
    Ok(crate::wire::diff_response(&diff))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    ahead: Vec<CommitSummary>,
    behind: Vec<CommitSummary>,
}

#[tauri::command]
pub async fn compare_branch(state: State<'_, AppState>, id: String, other: String) -> CmdResult<Comparison> {
    Ok(state
        .engine
        .read(&id, move |repo| {
            let cli = repo.cli();
            Ok(Comparison { ahead: cli.range_log(&other, "HEAD", 500)?, behind: cli.range_log("HEAD", &other, 500)? })
        })
        .await?)
}

#[tauri::command]
pub async fn worktrees(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Worktree>> {
    Ok(state.engine.read(&id, |repo| repo.cli().worktrees()).await?)
}

#[tauri::command]
pub async fn add_worktree(
    state: State<'_, AppState>,
    id: String,
    path: String,
    branch: String,
    create: bool,
) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().add_worktree(&PathBuf::from(&path), &branch, create)).await?)
}

#[tauri::command]
pub async fn remove_worktree(state: State<'_, AppState>, id: String, path: String, force: bool) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().remove_worktree(&path, force)).await?)
}

#[tauri::command]
pub async fn commit_patch(state: State<'_, AppState>, id: String, sha: String) -> CmdResult<String> {
    Ok(state.engine.read(&id, move |repo| repo.cli().format_patch(&sha)).await?)
}

#[tauri::command]
pub async fn apply_patch(state: State<'_, AppState>, id: String, patch: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().apply_patch(&patch)).await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePair {
    old: Option<String>,
    new: Option<String>,
    old_size: usize,
    new_size: usize,
}

const IMAGE_LIMIT: usize = 20 * 1024 * 1024;

fn mime(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => return None,
    })
}

fn data_url(mime: &str, bytes: Option<Vec<u8>>) -> (Option<String>, usize) {
    match bytes {
        Some(b) if b.len() <= IMAGE_LIMIT => {
            let len = b.len();
            (Some(format!("data:{mime};base64,{}", STANDARD.encode(b))), len)
        }
        Some(b) => (None, b.len()),
        None => (None, 0),
    }
}

#[tauri::command]
pub async fn image_pair(
    state: State<'_, AppState>,
    id: String,
    path: String,
    old_path: Option<String>,
    sha: Option<String>,
) -> CmdResult<ImagePair> {
    let mime = mime(&path).ok_or_else(|| CommandError::new("notImage", "Not an image"))?;
    Ok(state
        .engine
        .read(&id, move |repo| {
            let old_path = old_path.as_deref().unwrap_or(&path);
            let (old_src, new_src) = match &sha {
                Some(sha) => (BlobSource::Revision(format!("{sha}^")), BlobSource::Revision(sha.clone())),
                None => (BlobSource::Head, BlobSource::Worktree),
            };
            let (old, old_size) = data_url(mime, repo.read_blob(&old_src, old_path)?);
            let (new, new_size) = data_url(mime, repo.read_blob(&new_src, &path)?);
            Ok(ImagePair { old, new, old_size, new_size })
        })
        .await?)
}

#[tauri::command]
pub async fn read_worktree_file(state: State<'_, AppState>, id: String, path: String) -> CmdResult<String> {
    let bytes = state.engine.read(&id, move |repo| repo.read_blob(&BlobSource::Worktree, &path)).await?;
    Ok(String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned())
}

#[tauri::command]
pub async fn write_resolved_file(
    state: State<'_, AppState>,
    id: String,
    path: String,
    content: String,
) -> CmdResult<()> {
    Ok(state
        .engine
        .write(&id, move |repo| {
            std::fs::write(repo.workdir().join(&path), content)?;
            repo.cli().stage(&[&path])
        })
        .await?)
}

#[tauri::command]
pub async fn conflict_versions(state: State<'_, AppState>, id: String, path: String) -> CmdResult<[Option<String>; 3]> {
    Ok(state
        .engine
        .read(&id, move |repo| {
            let cli = repo.cli();
            let stage = |n: u8| cli.run(&["show", &format!(":{n}:{path}")]).ok();
            Ok([stage(1), stage(2), stage(3)])
        })
        .await?)
}
