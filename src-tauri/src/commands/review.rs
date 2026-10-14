use blink_diff::{FileDiff, parse_unified};
use blink_github::{DraftComment, IssueComment, PullDetail, PullFile, Review, ReviewComment, ReviewEvent};
use serde::Serialize;
use tauri::State;

use crate::commands::github::{HostedRepo, host_client, hosted, hosted_repo};
use crate::error::CmdResult;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFile {
    path: String,
    old_path: Option<String>,
    status: String,
    diff: FileDiff,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReview {
    detail: PullDetail,
    files: Vec<ReviewFile>,
    comments: Vec<IssueComment>,
    review_comments: Vec<ReviewComment>,
    reviews: Vec<Review>,
}

fn review_file(file: PullFile) -> ReviewFile {
    let diff = match &file.patch {
        Some(patch) => parse_unified(patch),
        None if file.additions + file.deletions > 0 => {
            FileDiff { too_large: true, additions: file.additions, deletions: file.deletions, ..Default::default() }
        }
        None => FileDiff { binary: file.status != "renamed", ..Default::default() },
    };
    ReviewFile { path: file.filename, old_path: file.previous_filename, status: file.status, diff }
}

fn target(state: &AppState, id: &str) -> CmdResult<HostedRepo> {
    let handle = state.engine.get(id)?;
    let repo = handle.repo();
    hosted_repo(state, repo, Some("upstream")).map(Ok).unwrap_or_else(|| hosted(state, repo))
}

#[tauri::command]
pub async fn pull_request_review(state: State<'_, AppState>, id: String, number: u64) -> CmdResult<PullRequestReview> {
    let HostedRepo { host, owner, name } = target(&state, &id)?;
    let gh = host_client(&state, &host, &id)?;
    let (detail, files, comments, review_comments, reviews) = tokio::try_join!(
        gh.pull(&owner, &name, number),
        gh.pull_files(&owner, &name, number),
        gh.issue_comments(&owner, &name, number),
        gh.review_comments(&owner, &name, number),
        gh.reviews(&owner, &name, number),
    )?;
    Ok(PullRequestReview {
        detail,
        files: files.into_iter().map(review_file).collect(),
        comments,
        review_comments,
        reviews,
    })
}

#[tauri::command]
pub async fn submit_pull_request_review(
    state: State<'_, AppState>,
    id: String,
    number: u64,
    event: ReviewEvent,
    body: String,
    comments: Vec<DraftComment>,
) -> CmdResult<Review> {
    let HostedRepo { host, owner, name } = target(&state, &id)?;
    Ok(host_client(&state, &host, &id)?.submit_review(&owner, &name, number, event, body.trim(), &comments).await?)
}

#[tauri::command]
pub async fn comment_on_pull_request(
    state: State<'_, AppState>,
    id: String,
    number: u64,
    body: String,
) -> CmdResult<IssueComment> {
    let HostedRepo { host, owner, name } = target(&state, &id)?;
    Ok(host_client(&state, &host, &id)?.add_issue_comment(&owner, &name, number, body.trim()).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(patch: Option<&str>, status: &str, additions: u32) -> PullFile {
        PullFile {
            filename: "a.rs".into(),
            previous_filename: None,
            status: status.into(),
            additions,
            deletions: 0,
            patch: patch.map(str::to_string),
        }
    }

    #[test]
    fn files_without_a_patch_are_binary_large_or_renames() {
        assert_eq!(review_file(file(Some("@@ -0,0 +1 @@\n+x"), "added", 1)).diff.additions, 1);
        assert!(review_file(file(None, "modified", 9000)).diff.too_large);
        assert!(review_file(file(None, "modified", 0)).diff.binary);
        let renamed = review_file(file(None, "renamed", 0)).diff;
        assert!(!renamed.binary && renamed.hunks.is_empty());
    }
}
