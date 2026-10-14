use std::time::Instant;

use blink_git::{AheadBehind, HeadState, RepoStatus};

#[derive(Debug, Clone)]
pub struct RepoSnapshot {
    pub generation: u64,
    pub head: Option<HeadState>,
    pub status: RepoStatus,
    pub ahead_behind: Option<AheadBehind>,
    pub operation: Option<&'static str>,
    pub has_remote: bool,
    pub bisecting: bool,
    pub taken_at: Instant,
}

impl RepoSnapshot {
    pub fn empty() -> Self {
        Self {
            generation: 0,
            head: None,
            status: RepoStatus::default(),
            ahead_behind: None,
            operation: None,
            has_remote: false,
            bisecting: false,
            taken_at: Instant::now(),
        }
    }
}
