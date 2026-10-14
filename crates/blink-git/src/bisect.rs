use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::error::Result;
use crate::validate::validate_sha;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BisectMark {
    Good,
    Bad,
    Skip,
}

impl BisectMark {
    fn command(self) -> &'static str {
        match self {
            BisectMark::Good => "good",
            BisectMark::Bad => "bad",
            BisectMark::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BisectState {
    pub bad: Option<String>,
    pub good: Vec<String>,
    pub skipped: Vec<String>,
    pub current: Option<String>,
    pub current_summary: Option<String>,
    pub revisions_left: Option<u32>,
    pub steps_left: Option<u32>,
    pub first_bad: Option<String>,
    pub first_bad_summary: Option<String>,
}

pub fn is_bisecting(git_dir: &Path) -> bool {
    git_dir.join("BISECT_START").is_file()
}

impl GitCli {
    pub fn bisect_start(&self, bad: &str, good: Option<&str>) -> Result<()> {
        let mut args = vec!["bisect", "start", validate_sha(bad)?];
        if let Some(good) = good {
            args.push(validate_sha(good)?);
        }
        args.push("--");
        self.run(&args).map(drop)
    }

    pub fn bisect_mark(&self, mark: BisectMark, sha: Option<&str>) -> Result<()> {
        let mut args = vec!["bisect", mark.command()];
        if let Some(sha) = sha {
            args.push(validate_sha(sha)?);
        }
        self.run(&args).map(drop)
    }

    pub fn bisect_reset(&self) -> Result<()> {
        self.run(&["bisect", "reset"]).map(drop)
    }

    pub fn bisect_state(&self, git_dir: &Path) -> Result<Option<BisectState>> {
        if !is_bisecting(git_dir) {
            return Ok(None);
        }
        let (bad_term, good_term) = terms(git_dir);
        let refs = self.run(&["for-each-ref", "--format=%(refname) %(objectname)", "refs/bisect/"])?;
        let mut state = BisectState::default();
        for (name, sha) in refs.lines().filter_map(|l| l.split_once(' ')) {
            let Some(name) = name.strip_prefix("refs/bisect/") else { continue };
            if name == bad_term {
                state.bad = Some(sha.to_string());
            } else if name.starts_with(&format!("{good_term}-")) {
                state.good.push(sha.to_string());
            } else if name.starts_with("skip-") {
                state.skipped.push(sha.to_string());
            }
        }
        state.current = self.run(&["rev-parse", "--verify", "-q", "HEAD"]).ok().map(|s| s.trim().to_string());
        state.current_summary = state.current.as_deref().and_then(|sha| self.subject(sha));
        if let Some(bad) = state.bad.clone()
            && !state.good.is_empty()
        {
            let mut args = vec!["rev-list", "--bisect-vars", bad.as_str(), "--not"];
            args.extend(state.good.iter().map(String::as_str));
            let vars = parse_bisect_vars(&self.run(&args)?);
            if vars.all == Some(1) {
                state.first_bad_summary = self.subject(&bad);
                state.first_bad = Some(bad);
            } else {
                state.revisions_left = vars.nr;
                state.steps_left = vars.steps;
            }
        }
        Ok(Some(state))
    }

    fn subject(&self, sha: &str) -> Option<String> {
        self.run(&["log", "-1", "--format=%s", sha, "--"]).ok().map(|s| s.trim().to_string())
    }
}

fn terms(git_dir: &Path) -> (String, String) {
    let text = std::fs::read_to_string(git_dir.join("BISECT_TERMS")).unwrap_or_default();
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    match (lines.next(), lines.next()) {
        (Some(bad), Some(good)) => (bad.to_string(), good.to_string()),
        _ => ("bad".into(), "good".into()),
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct BisectVars {
    nr: Option<u32>,
    steps: Option<u32>,
    all: Option<u32>,
}

fn parse_bisect_vars(output: &str) -> BisectVars {
    let mut vars = BisectVars::default();
    for (key, value) in output.lines().filter_map(|l| l.split_once('=')) {
        let value = value.trim_matches('\'').parse().ok();
        match key {
            "bisect_nr" => vars.nr = value,
            "bisect_steps" => vars.steps = value,
            "bisect_all" => vars.all = value,
            _ => {}
        }
    }
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rev_list_bisect_vars() {
        let out = "bisect_rev='abc'\nbisect_nr=3\nbisect_good=2\nbisect_bad=1\nbisect_all=7\nbisect_steps=2\n";
        assert_eq!(parse_bisect_vars(out), BisectVars { nr: Some(3), steps: Some(2), all: Some(7) });
        assert_eq!(parse_bisect_vars(""), BisectVars::default());
    }

    #[test]
    fn falls_back_to_default_terms() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(terms(dir.path()), ("bad".into(), "good".into()));
        std::fs::write(dir.path().join("BISECT_TERMS"), "new\nold\n").unwrap();
        assert_eq!(terms(dir.path()), ("new".into(), "old".into()));
    }
}
