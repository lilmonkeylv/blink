use blink_core::undo::UndoEntry;
use blink_git::{Operation, RebaseAction, RebaseStep, ResetMode, validate_sha};
use serde::Deserialize;
use tauri::State;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

fn validate_all<'a>(shas: impl IntoIterator<Item = &'a String>) -> CmdResult<()> {
    for sha in shas {
        validate_sha(sha)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn revert_commit(state: State<'_, AppState>, id: String, sha: String) -> CmdResult<()> {
    validate_sha(&sha)?;
    let label = format!("Revert {}", short(&sha));
    Ok(state.engine.rewrite(&id, label, move |repo| repo.cli().revert(&sha)).await?)
}

#[tauri::command]
pub async fn cherry_pick(
    state: State<'_, AppState>,
    id: String,
    shas: Vec<String>,
    target: Option<String>,
) -> CmdResult<()> {
    validate_all(&shas)?;
    let label = format!("Cherry-pick {} commit{}", shas.len(), if shas.len() == 1 { "" } else { "s" });
    Ok(state
        .engine
        .rewrite(&id, label, move |repo| {
            let cli = repo.cli();
            if let Some(branch) = &target {
                cli.checkout(branch)?;
            }
            let refs: Vec<&str> = shas.iter().map(String::as_str).collect();
            cli.cherry_pick(&refs)
        })
        .await?)
}

#[tauri::command]
pub async fn reset_to_commit(state: State<'_, AppState>, id: String, sha: String, mode: ResetMode) -> CmdResult<()> {
    validate_sha(&sha)?;
    let label = format!("Reset to {}", short(&sha));
    Ok(state.engine.rewrite(&id, label, move |repo| repo.cli().reset(&sha, mode)).await?)
}

#[tauri::command]
pub async fn checkout_commit(state: State<'_, AppState>, id: String, sha: String) -> CmdResult<()> {
    validate_sha(&sha)?;
    Ok(state.engine.write(&id, move |repo| repo.cli().checkout(&sha)).await?)
}

#[tauri::command]
pub async fn tags(state: State<'_, AppState>, id: String) -> CmdResult<Vec<(String, String)>> {
    Ok(state.engine.read(&id, |repo| repo.cli().tags()).await?)
}

#[tauri::command]
pub async fn create_tag(
    state: State<'_, AppState>,
    id: String,
    name: String,
    sha: String,
    message: Option<String>,
) -> CmdResult<()> {
    validate_sha(&sha)?;
    Ok(state.engine.write(&id, move |repo| repo.cli().create_tag(&name, &sha, message.as_deref())).await?)
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, id: String, name: String) -> CmdResult<()> {
    Ok(state.engine.write(&id, move |repo| repo.cli().delete_tag(&name)).await?)
}

#[tauri::command]
pub async fn rebase_branch(state: State<'_, AppState>, id: String, onto: String) -> CmdResult<()> {
    let label = format!("Rebase onto {onto}");
    Ok(state.engine.rewrite(&id, label, move |repo| repo.cli().rebase_onto(&onto)).await?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlan {
    base: Option<String>,
    steps: Vec<RebaseStep>,
    label: Option<String>,
}

fn ensure_linear(repo: &blink_git::Repository, steps: &[RebaseStep]) -> blink_git::Result<()> {
    for step in steps {
        let detail = repo.commit_detail(&step.sha)?;
        if detail.commit.parents.len() > 1 {
            return Err(blink_git::Error::Git(format!(
                "{} is a merge commit; rewriting history across merges is not supported",
                short(&step.sha)
            )));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn rebase_interactive(state: State<'_, AppState>, id: String, plan: RebasePlan) -> CmdResult<()> {
    if plan.steps.is_empty() {
        return Err(CommandError::new("empty", "Nothing to rewrite"));
    }
    validate_all(plan.base.iter().chain(plan.steps.iter().map(|s| &s.sha)))?;
    let label = plan.label.clone().unwrap_or_else(|| "Rewrite history".into());
    Ok(state
        .engine
        .rewrite(&id, label, move |repo| {
            ensure_linear(repo, &plan.steps)?;
            repo.cli().rebase_interactive(plan.base.as_deref(), &plan.steps)
        })
        .await?)
}

#[tauri::command]
pub async fn squash_commits(
    state: State<'_, AppState>,
    id: String,
    target: String,
    commits: Vec<String>,
    message: String,
) -> CmdResult<()> {
    validate_all(std::iter::once(&target).chain(&commits))?;
    let label = format!("Squash {} commits", commits.len() + 1);
    Ok(state
        .engine
        .rewrite(&id, label, move |repo| {
            let mut all = vec![target.clone()];
            all.extend(commits.iter().cloned());
            let history = repo.log(None, 0, 5_000)?;
            let oldest_index = all
                .iter()
                .filter_map(|sha| history.iter().position(|c| c.id == *sha))
                .max()
                .ok_or_else(|| blink_git::Error::Git("commits are not on the current branch".into()))?;
            let base = history[oldest_index].parents.first().cloned();
            let range: Vec<_> = history[..=oldest_index].iter().rev().collect();
            let mut steps: Vec<RebaseStep> = Vec::new();
            for c in &range {
                if commits.contains(&c.id) {
                    continue;
                }
                steps.push(RebaseStep { action: RebaseAction::Pick, sha: c.id.clone(), message: None });
                if c.id == target {
                    for sha in range.iter().filter(|r| commits.contains(&r.id)) {
                        steps.push(RebaseStep { action: RebaseAction::Squash, sha: sha.id.clone(), message: None });
                    }
                }
            }
            if let Some(last) = steps.iter_mut().rev().find(|s| s.action == RebaseAction::Squash) {
                last.message = Some(message.clone());
            }
            ensure_linear(repo, &steps)?;
            repo.cli().rebase_interactive(base.as_deref(), &steps)
        })
        .await?)
}

#[tauri::command]
pub async fn continue_operation(state: State<'_, AppState>, id: String, operation: String) -> CmdResult<()> {
    let op =
        Operation::from_name(&operation).ok_or_else(|| CommandError::new("unknownOperation", operation.clone()))?;
    Ok(state.engine.write(&id, move |repo| repo.cli().continue_operation(op)).await?)
}

#[tauri::command]
pub async fn abort_operation(state: State<'_, AppState>, id: String, operation: String) -> CmdResult<()> {
    let op =
        Operation::from_name(&operation).ok_or_else(|| CommandError::new("unknownOperation", operation.clone()))?;
    Ok(state.engine.write(&id, move |repo| repo.cli().abort_operation(op)).await?)
}

#[tauri::command]
pub async fn skip_operation(state: State<'_, AppState>, id: String, operation: String) -> CmdResult<()> {
    let op =
        Operation::from_name(&operation).ok_or_else(|| CommandError::new("unknownOperation", operation.clone()))?;
    Ok(state.engine.write(&id, move |repo| repo.cli().skip_operation(op)).await?)
}

#[tauri::command]
pub fn undo_history(state: State<'_, AppState>, id: String) -> CmdResult<Vec<UndoEntry>> {
    Ok(state.engine.get(&id)?.undo().entries())
}

#[tauri::command]
pub async fn undo(state: State<'_, AppState>, id: String) -> CmdResult<UndoEntry> {
    let handle = state.engine.get(&id)?;
    Ok(state.engine.write(&id, move |repo| handle.undo().undo(repo)).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_never_splits_a_character() {
        assert_eq!(short("0123456789abcdef"), "0123456");
        assert_eq!(short("abc"), "abc");
        assert_eq!(short("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}"), "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}");
    }

    #[test]
    fn rejects_non_hex_shas() {
        let shas = ["abc1234".to_string(), "--output=x".to_string()];
        assert!(validate_all(&shas[..1]).is_ok());
        assert_eq!(validate_all(&shas).unwrap_err().kind, "invalidArgument");
    }
}
