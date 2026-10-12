use blink_git::{Branch, BranchKind};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn branches(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Branch>> {
    Ok(state.engine.read(&id, |repo| repo.branches()).await?)
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UncommittedChanges {
    BringAlong,
    StashOnCurrent,
}

#[tauri::command]
pub async fn checkout_branch(
    state: State<'_, AppState>,
    id: String,
    name: String,
    kind: BranchKind,
    changes: Option<UncommittedChanges>,
) -> CmdResult<()> {
    Ok(state
        .engine
        .write(&id, move |repo| {
            let cli = repo.cli();
            if matches!(changes, Some(UncommittedChanges::StashOnCurrent)) {
                let current = repo.head()?.branch_name().unwrap_or("HEAD").to_string();
                cli.stash_push(&format!("!!Blink<{current}>"), &[])?;
            }
            match kind {
                BranchKind::Local => cli.checkout(&name)?,
                BranchKind::Remote => {
                    let local = name.split_once('/').map(|(_, b)| b).unwrap_or(&name);
                    let exists = repo.branches()?.iter().any(|b| b.kind == BranchKind::Local && b.name == local);
                    if exists { cli.checkout(local)? } else { cli.checkout_remote_branch(&name, local)? }
                }
            }
            if let Some(target) = repo.head()?.branch_name()
                && let Some((stash, _)) =
                    cli.stash_list()?.into_iter().find(|(_, msg)| msg.ends_with(&format!("!!Blink<{target}>")))
            {
                cli.stash_pop(&stash)?;
            }
            Ok(())
        })
        .await?)
}

#[tauri::command]
pub async fn create_branch(
    state: State<'_, AppState>,
    id: String,
    name: String,
    start: Option<String>,
    checkout: Option<bool>,
) -> CmdResult<()> {
    let checkout = checkout.unwrap_or(true);
    Ok(state.engine.write(&id, move |repo| repo.cli().create_branch(&name, start.as_deref(), checkout)).await?)
}

#[tauri::command]
pub async fn rename_branch(state: State<'_, AppState>, id: String, old: String, new: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().rename_branch(&old, &new)).await?)
}

#[tauri::command]
pub async fn delete_branch(
    state: State<'_, AppState>,
    id: String,
    name: String,
    force: Option<bool>,
    include_remote: Option<bool>,
) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    Ok(state
        .engine
        .write(&id, move |repo| {
            let branch = repo.branches()?.into_iter().find(|b| b.name == name && b.kind == BranchKind::Local);
            let upstream = branch.as_ref().and_then(|b| b.upstream.clone());
            cli.delete_branch(&name, force.unwrap_or(false))?;
            if let Some(b) = &branch {
                handle.undo().record_branch_delete(repo, &name, &b.target)?;
            }
            if include_remote.unwrap_or(false)
                && let Some((remote, branch)) = upstream.as_deref().and_then(|u| u.split_once('/'))
            {
                cli.delete_remote_branch(remote, branch)?;
            }
            Ok(())
        })
        .await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeOutcome {
    conflicted: bool,
    output: String,
}

#[tauri::command]
pub async fn merge_branch(
    state: State<'_, AppState>,
    id: String,
    branch: String,
    squash: Option<bool>,
) -> CmdResult<MergeOutcome> {
    let squash = squash.unwrap_or(false);
    let label = format!("Merge {branch}");
    let result = state.engine.rewrite(&id, label, move |repo| repo.cli().merge(&branch, squash)).await;
    match result {
        Ok(output) => Ok(MergeOutcome { conflicted: false, output }),
        Err(blink_core::Error::Git(blink_git::Error::Cli { stderr, .. })) => {
            let snapshot = state.engine.get(&id)?.snapshot();
            if snapshot.status.conflicted > 0 {
                Ok(MergeOutcome { conflicted: true, output: stderr })
            } else {
                Err(blink_git::Error::Cli { command: "merge".into(), stderr, code: None }.into())
            }
        }
        Err(e) => Err(e.into()),
    }
}

#[tauri::command]
pub async fn abort_merge(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, |repo| repo.cli().merge_abort()).await?)
}

#[tauri::command]
pub async fn resolve_conflict(state: State<'_, AppState>, id: String, path: String, ours: bool) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().checkout_conflict_side(&path, ours)).await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    reference: String,
    message: String,
}

#[tauri::command]
pub async fn stash_changes(
    state: State<'_, AppState>,
    id: String,
    message: Option<String>,
    paths: Option<Vec<String>>,
) -> CmdResult<()> {
    Ok(state
        .engine
        .write(&id, move |repo| {
            let label = match message {
                Some(m) if !m.trim().is_empty() => m,
                _ => format!("!!Blink<{}>", repo.head()?.branch_name().unwrap_or("HEAD")),
            };
            let paths = paths.unwrap_or_default();
            let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
            repo.cli().stash_push(&label, &refs)
        })
        .await?)
}

#[tauri::command]
pub async fn stash_list(state: State<'_, AppState>, id: String) -> CmdResult<Vec<StashEntry>> {
    let entries = state.engine.read(&id, |repo| repo.cli().stash_list()).await?;
    Ok(entries.into_iter().map(|(reference, message)| StashEntry { reference, message }).collect())
}

#[tauri::command]
pub async fn stash_pop(state: State<'_, AppState>, id: String, reference: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().stash_pop(&reference)).await?)
}

#[tauri::command]
pub async fn stash_drop(state: State<'_, AppState>, id: String, reference: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().stash_drop(&reference)).await?)
}
