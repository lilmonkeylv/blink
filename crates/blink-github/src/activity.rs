use serde::Deserialize;

use crate::api::Client;
use crate::error::Result;

const ACTIVITY_QUERY: &str = "query($owner: String!, $name: String!, $branch: String!, $search: String!) {
  repository(owner: $owner, name: $name) {
    pullRequests(headRefName: $branch, states: [OPEN, MERGED], first: 1, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes { number title state }
    }
  }
  search(query: $search, type: ISSUE, first: 20) {
    nodes { ... on PullRequest { number title } }
  }
}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullBrief {
    pub number: u64,
    pub title: String,
    pub merged: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullActivity {
    pub branch_pull: Option<PullBrief>,
    pub review_requests: Vec<PullBrief>,
}

#[derive(Deserialize)]
struct ActivityData {
    repository: Option<ActivityRepository>,
    search: Nodes<Node>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivityRepository {
    pull_requests: Nodes<Node>,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
struct Node {
    number: Option<u64>,
    title: Option<String>,
    state: Option<String>,
}

impl Node {
    fn brief(self) -> Option<PullBrief> {
        Some(PullBrief { number: self.number?, title: self.title?, merged: self.state.as_deref() == Some("MERGED") })
    }
}

impl From<ActivityData> for PullActivity {
    fn from(data: ActivityData) -> Self {
        Self {
            branch_pull: data.repository.and_then(|r| r.pull_requests.nodes.into_iter().next()).and_then(Node::brief),
            review_requests: data.search.nodes.into_iter().filter_map(Node::brief).collect(),
        }
    }
}

impl Client {
    pub async fn pull_activity(&self, owner: &str, name: &str, branch: &str) -> Result<PullActivity> {
        let search = format!("repo:{owner}/{name} is:pr is:open review-requested:@me");
        let variables = serde_json::json!({ "owner": owner, "name": name, "branch": branch, "search": search });
        Ok(self.graphql::<ActivityData>(ACTIVITY_QUERY, variables).await?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_pull_and_review_requests() {
        let data: ActivityData = serde_json::from_str(
            r#"{
              "repository": { "pullRequests": { "nodes": [{ "number": 7, "title": "Fix it", "state": "MERGED" }] } },
              "search": { "nodes": [{ "number": 9, "title": "Review me" }, {}] }
            }"#,
        )
        .unwrap();
        let activity = PullActivity::from(data);
        assert_eq!(activity.branch_pull, Some(PullBrief { number: 7, title: "Fix it".into(), merged: true }));
        assert_eq!(activity.review_requests, [PullBrief { number: 9, title: "Review me".into(), merged: false }]);

        let empty: ActivityData = serde_json::from_str(r#"{ "repository": null, "search": { "nodes": [] } }"#).unwrap();
        assert_eq!(PullActivity::from(empty), PullActivity::default());
    }
}
