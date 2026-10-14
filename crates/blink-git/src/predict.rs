use serde::Serialize;

use crate::cli::GitCli;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum MergePrediction {
    UpToDate,
    FastForward,
    Clean,
    Conflicts { files: Vec<String> },
    Unknown,
}

impl GitCli {
    pub fn predict_merge(&self, branch: &str) -> Result<MergePrediction> {
        if branch.is_empty() || branch.starts_with('-') {
            return Err(Error::InvalidArgument(format!("{branch:?} is not a branch")));
        }
        let Some(ours) = self.commit_id("HEAD") else { return Ok(MergePrediction::Unknown) };
        let theirs = self
            .commit_id(branch)
            .ok_or_else(|| Error::InvalidArgument(format!("{branch} does not point at a commit")))?;
        if ours == theirs || self.is_ancestor(&theirs, &ours)? {
            return Ok(MergePrediction::UpToDate);
        }
        if self.is_ancestor(&ours, &theirs)? {
            return Ok(MergePrediction::FastForward);
        }
        let output = self
            .command(&["merge-tree", "--write-tree", "--name-only", "--no-messages", &ours, &theirs])
            .output()
            .map_err(Self::spawn_error)?;
        Ok(match output.status.code() {
            Some(0) => MergePrediction::Clean,
            Some(1) => MergePrediction::Conflicts { files: conflicted_files(&String::from_utf8_lossy(&output.stdout)) },
            _ => MergePrediction::Unknown,
        })
    }

    fn commit_id(&self, rev: &str) -> Option<String> {
        let spec = format!("{rev}^{{commit}}");
        let out = self.run(&["rev-parse", "--verify", "-q", "--end-of-options", &spec]).ok()?;
        Some(out.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        match self.run(&["merge-base", "--is-ancestor", ancestor, descendant]) {
            Ok(_) => Ok(true),
            Err(Error::Cli { code: Some(1), .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

fn conflicted_files(stdout: &str) -> Vec<String> {
    stdout.lines().skip(1).take_while(|l| !l.is_empty()).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_conflicted_names_after_the_tree_id() {
        let out = "4b825dc642cb6eb9a060e54bf8d69288fbee4904\nsrc/a.rs\nb c.txt\n\nAuto-merging src/a.rs\n";
        assert_eq!(conflicted_files(out), ["src/a.rs", "b c.txt"]);
        assert!(conflicted_files("4b825dc642cb6eb9a060e54bf8d69288fbee4904\n").is_empty());
    }
}
