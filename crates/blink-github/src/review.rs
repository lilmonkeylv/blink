use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::api::{Client, Owner};
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullFile {
    pub filename: String,
    pub previous_filename: Option<String>,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: u64,
    pub path: String,
    pub line: Option<u32>,
    pub original_line: Option<u32>,
    pub side: Option<String>,
    pub body: String,
    pub user: Owner,
    pub created_at: String,
    pub in_reply_to_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueComment {
    pub id: u64,
    pub body: String,
    pub user: Owner,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub id: u64,
    pub user: Owner,
    pub state: String,
    pub body: Option<String>,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullDetail {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub user: Owner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftComment {
    pub path: String,
    pub line: u32,
    pub side: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewEvent {
    Approve,
    RequestChanges,
    Comment,
}

#[derive(Serialize)]
struct NewReview<'a> {
    event: ReviewEvent,
    #[serde(skip_serializing_if = "str::is_empty")]
    body: &'a str,
    comments: &'a [DraftComment],
}

#[derive(Serialize)]
struct NewComment<'a> {
    body: &'a str,
}

impl Client {
    pub async fn pull(&self, owner: &str, name: &str, number: u64) -> Result<PullDetail> {
        self.get(&format!("/repos/{owner}/{name}/pulls/{number}")).await
    }

    pub async fn pull_files(&self, owner: &str, name: &str, number: u64) -> Result<Vec<PullFile>> {
        self.get_all(&format!("/repos/{owner}/{name}/pulls/{number}/files?per_page=100"), 30).await
    }

    pub async fn review_comments(&self, owner: &str, name: &str, number: u64) -> Result<Vec<ReviewComment>> {
        self.get_all(&format!("/repos/{owner}/{name}/pulls/{number}/comments?per_page=100"), 20).await
    }

    pub async fn issue_comments(&self, owner: &str, name: &str, number: u64) -> Result<Vec<IssueComment>> {
        self.get_all(&format!("/repos/{owner}/{name}/issues/{number}/comments?per_page=100"), 20).await
    }

    pub async fn reviews(&self, owner: &str, name: &str, number: u64) -> Result<Vec<Review>> {
        self.get_all(&format!("/repos/{owner}/{name}/pulls/{number}/reviews?per_page=100"), 10).await
    }

    pub async fn submit_review(
        &self,
        owner: &str,
        name: &str,
        number: u64,
        event: ReviewEvent,
        body: &str,
        comments: &[DraftComment],
    ) -> Result<Review> {
        let review = NewReview { event, body, comments };
        self.send(Method::POST, &format!("/repos/{owner}/{name}/pulls/{number}/reviews"), &review).await
    }

    pub async fn add_issue_comment(&self, owner: &str, name: &str, number: u64, body: &str) -> Result<IssueComment> {
        self.send(Method::POST, &format!("/repos/{owner}/{name}/issues/{number}/comments"), &NewComment { body }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_payload_matches_the_api() {
        let comments = [DraftComment { path: "a.rs".into(), line: 3, side: "RIGHT".into(), body: "nit".into() }];
        let json =
            serde_json::to_value(NewReview { event: ReviewEvent::RequestChanges, body: "", comments: &comments })
                .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "event": "REQUEST_CHANGES", "comments": [{ "path": "a.rs", "line": 3, "side": "RIGHT", "body": "nit" }] })
        );
    }
}
