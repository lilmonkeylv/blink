use blink_core::ops::{self, FileSelection};
use blink_git::{Branch, BranchKind, MergePrediction};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

#[tauri::command]
pub async fn branches(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Branch>> {
    Ok(state.engine.read(&id, |repo| repo.branches()).await?)
}

#[tauri::command]
pub async fn unpushed_commits(state: State<'_, AppState>, id: String, name: String) -> CmdResult<usize> {
    Ok(state.engine.read(&id, move |repo| unpushed(&repo.cli(), &name)).await?)
}

fn unpushed(cli: &blink_git::GitCli, branch: &str) -> blink_git::Result<usize> {
    let tip = format!("refs/heads/{branch}");
    let count = cli.run(&["rev-list", "--count", &tip, "--not", "--remotes", "--"])?;
    Ok(count.trim().parse().unwrap_or(0))
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
    let include_remote = include_remote.unwrap_or(false);
    let delete = move |repo: &blink_git::Repository| {
        let branch = repo.branches()?.into_iter().find(|b| b.name == name && b.kind == BranchKind::Local);
        let upstream = branch.as_ref().and_then(|b| b.upstream.clone());
        cli.delete_branch(&name, force.unwrap_or(false))?;
        if let Some(b) = &branch {
            handle.undo().record_branch_delete(repo, &name, &b.target)?;
        }
        if include_remote && let Some((remote, branch)) = upstream.as_deref().and_then(|u| u.split_once('/')) {
            cli.delete_remote_branch(remote, branch)?;
        }
        Ok(())
    };
    if include_remote {
        Ok(state.engine.network_write(&id, delete).await?)
    } else {
        Ok(state.engine.write(&id, delete).await?)
    }
}

#[tauri::command]
pub async fn delete_remote_branch(state: State<'_, AppState>, id: String, name: String) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    let (remote, branch) =
        name.split_once('/').ok_or_else(|| CommandError::new("git", format!("{name} is not a remote branch")))?;
    let (remote, branch) = (remote.to_string(), branch.to_string());
    Ok(state.engine.network_write(&id, move |_| cli.delete_remote_branch(&remote, &branch)).await?)
}

#[tauri::command]
pub async fn set_upstream(
    state: State<'_, AppState>,
    id: String,
    branch: String,
    upstream: Option<String>,
) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().set_upstream(&branch, upstream.as_deref())).await?)
}

#[tauri::command]
pub async fn prune_remotes(state: State<'_, AppState>, id: String) -> CmdResult<Vec<String>> {
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    Ok(state
        .engine
        .write(&id, move |_| {
            let mut pruned = Vec::new();
            for (remote, _) in cli.remotes()? {
                pruned.extend(cli.prune_remote(&remote)?);
            }
            Ok(pruned)
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
pub async fn predict_merge(state: State<'_, AppState>, id: String, branch: String) -> CmdResult<MergePrediction> {
    Ok(state.engine.read(&id, move |repo| repo.cli().predict_merge(&branch)).await?)
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
pub async fn stash_selection(
    state: State<'_, AppState>,
    id: String,
    files: Vec<FileSelection>,
    message: String,
) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| ops::stash_selection(repo, &files, &message)).await?)
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

#[cfg(test)]
mod tests {
    use blink_git::GitCli;

    use super::*;

    #[test]
    fn counts_commits_missing_from_every_remote() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        GitCli::init(&work, "main").unwrap();
        let cli = GitCli::new(&work);
        for (k, v) in [("user.name", "T"), ("user.email", "t@example.com"), ("commit.gpgsign", "false")] {
            cli.run(&["config", k, v]).unwrap();
        }
        let commit = |msg: &str| cli.run(&["commit", "--allow-empty", "-q", "-m", msg]).unwrap();
        commit("one");
        cli.run(&["checkout", "-q", "-b", "feature"]).unwrap();
        commit("two");
        assert_eq!(unpushed(&cli, "feature").unwrap(), 2);

        let remote = dir.path().join("remote.git");
        GitCli::new(dir.path()).run(&["init", "-q", "--bare", remote.to_str().unwrap()]).unwrap();
        cli.run(&["remote", "add", "origin", remote.to_str().unwrap()]).unwrap();
        cli.run(&["push", "-q", "origin", "main"]).unwrap();
        assert_eq!(unpushed(&cli, "main").unwrap(), 0);
        assert_eq!(unpushed(&cli, "feature").unwrap(), 1);
    }
}
