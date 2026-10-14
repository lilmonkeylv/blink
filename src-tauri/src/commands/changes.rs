use blink_core::ops::{self, CommitMessage, CommitOptions, FileSelection};
use blink_diff::DiffOptions;
use blink_git::FileChange;
use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
use tauri::{AppHandle, State};

use crate::commands::hooks::{HookAction, hook_emitter};
use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

fn change_for(state: &AppState, id: &str, path: &str) -> CmdResult<FileChange> {
    state
        .engine
        .get(id)?
        .snapshot()
        .status
        .files
        .iter()
        .find(|f| f.path == path)
        .cloned()
        .ok_or_else(|| CommandError::new("stale", format!("{path} has no changes")))
}

#[tauri::command]
pub async fn file_diff(
    state: State<'_, AppState>,
    id: String,
    path: String,
    ignore_whitespace: Option<bool>,
    full: Option<bool>,
    highlight: Option<bool>,
) -> CmdResult<Response> {
    let change = change_for(&state, &id, &path)?;
    let opts = DiffOptions {
        ignore_whitespace: ignore_whitespace.unwrap_or(false),
        max_lines: if full.unwrap_or(false) { None } else { DiffOptions::default().max_lines },
        highlight: highlight.unwrap_or(true),
        ..Default::default()
    };
    let diff = state.engine.read(&id, move |repo| ops::working_diff(repo, &change, opts)).await?;
    Ok(crate::wire::diff_response(&diff))
}

#[tauri::command]
pub async fn commit(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    files: Vec<FileSelection>,
    message: CommitMessage,
    amend: Option<bool>,
    no_verify: Option<bool>,
) -> CmdResult<String> {
    if files.is_empty() && !amend.unwrap_or(false) {
        return Err(CommandError::new("nothingSelected", "Select at least one file to commit"));
    }
    let amend = amend.unwrap_or(false);
    let sign = state.settings.read().git.sign_commits;
    let options = CommitOptions { amend, sign, verify: !no_verify.unwrap_or(false) };
    let label = if amend { "Amend commit" } else { "Commit" };
    let emit = hook_emitter(&app, &id, HookAction::Commit);
    Ok(state
        .engine
        .rewrite(&id, label, move |repo| ops::commit_selection_with_hooks(repo, &files, &message, options, emit))
        .await?)
}

#[tauri::command]
pub async fn discard_files(state: State<'_, AppState>, id: String, paths: Vec<String>) -> CmdResult<()> {
    let changes: Vec<FileChange> = paths.iter().map(|p| change_for(&state, &id, p)).collect::<CmdResult<_>>()?;
    Ok(state.engine.write(&id, move |repo| ops::discard_files(repo, &changes)).await?)
}

#[tauri::command]
pub async fn discard_lines(
    state: State<'_, AppState>,
    id: String,
    path: String,
    lines: Vec<(usize, usize)>,
) -> CmdResult<()> {
    let change = change_for(&state, &id, &path)?;
    Ok(state.engine.write(&id, move |repo| ops::discard_lines(repo, &change, &lines)).await?)
}

#[tauri::command]
pub async fn ignore_path(state: State<'_, AppState>, id: String, pattern: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| ops::ignore(repo, &pattern)).await?)
}

#[tauri::command]
pub async fn undo_last_commit(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    Ok(state.engine.rewrite(&id, "Undo commit", |repo| repo.cli().undo_last_commit()).await?)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentity {
    name: Option<String>,
    email: Option<String>,
}

#[tauri::command]
pub async fn git_identity(state: State<'_, AppState>, id: Option<String>) -> CmdResult<GitIdentity> {
    let cli = match id {
        Some(id) => state.engine.get(&id)?.repo().cli(),
        None => blink_git::GitCli::new(&state.paths.data_dir),
    };
    Ok(GitIdentity { name: cli.config_get("user.name"), email: cli.config_get("user.email") })
}

#[tauri::command]
pub async fn commit_template(state: State<'_, AppState>, id: String) -> CmdResult<Option<String>> {
    let handle = state.engine.get(&id)?;
    let repo = handle.repo();
    let Ok(path) = repo.cli().run(&["config", "--path", "--get", "commit.template"]) else { return Ok(None) };
    let Ok(text) = std::fs::read_to_string(repo.workdir().join(path.trim())) else { return Ok(None) };
    Ok(Some(template_body(&text)).filter(|t| !t.is_empty()))
}

fn template_body(text: &str) -> String {
    text.lines().filter(|l| !l.starts_with('#')).collect::<Vec<_>>().join("\n").trim().to_string()
}

#[tauri::command]
pub async fn set_git_identity(state: State<'_, AppState>, identity: GitIdentity) -> CmdResult<()> {
    let cli = blink_git::GitCli::new(&state.paths.data_dir);
    if let Some(name) = identity.name.filter(|n| !n.trim().is_empty()) {
        cli.config_set_global("user.name", name.trim())?;
    }
    if let Some(email) = identity.email.filter(|e| !e.trim().is_empty()) {
        cli.config_set_global("user.email", email.trim())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_drops_comment_lines() {
        assert_eq!(template_body("\n# Why:\nTicket: \n\n# end\n"), "Ticket:");
        assert_eq!(template_body("# only comments\n"), "");
    }
}
