use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, Progress};
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    pub owner: Option<String>,
    pub locked_at: Option<String>,
}

#[derive(Deserialize)]
struct RawOwner {
    name: String,
}

#[derive(Deserialize)]
struct RawLock {
    id: String,
    path: String,
    owner: Option<RawOwner>,
    locked_at: Option<String>,
}

pub fn parse_lfs_locks(json: &str) -> Result<Vec<LfsLock>> {
    let raw: Vec<RawLock> =
        serde_json::from_str(json.trim()).map_err(|e| Error::Git(format!("unexpected git lfs locks output: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|l| LfsLock { id: l.id, path: l.path, owner: l.owner.map(|o| o.name), locked_at: l.locked_at })
        .collect())
}

pub fn uses_lfs(attributes: &str) -> bool {
    attributes
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .any(|l| l.split_whitespace().skip(1).any(|attr| attr == "filter=lfs"))
}

pub fn validate_lfs_pattern(pattern: &str) -> Result<&str> {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern.len() > 256 || pattern.starts_with('#') || pattern.contains(['\n', '\r', '\0']) {
        return Err(Error::InvalidArgument(format!("{pattern:?} is not a usable LFS pattern")));
    }
    Ok(pattern)
}

impl GitCli {
    pub fn lfs_version(&self) -> Option<String> {
        let out = self.run(&["lfs", "version"]).ok()?;
        Some(out.trim().to_string()).filter(|v| v.starts_with("git-lfs/"))
    }

    pub fn lfs_track(&self, pattern: &str) -> Result<()> {
        let pattern = validate_lfs_pattern(pattern)?;
        self.run(&["lfs", "track", "--", pattern]).map(drop)
    }

    pub fn lfs_pull(&self, progress: impl FnMut(Progress)) -> Result<()> {
        self.run_with_progress(&["lfs", "pull"], progress).map(drop)
    }

    pub fn lfs_locks(&self) -> Result<Vec<LfsLock>> {
        parse_lfs_locks(&self.run(&["lfs", "locks", "--json"])?)
    }

    pub fn lfs_lock(&self, path: &str) -> Result<()> {
        self.run(&["lfs", "lock", "--", path]).map(drop)
    }

    pub fn lfs_unlock(&self, path: &str, force: bool) -> Result<()> {
        let mut args = vec!["lfs", "unlock"];
        if force {
            args.push("--force");
        }
        args.extend(["--", path]);
        self.run(&args).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_lfs_attributes() {
        assert!(uses_lfs("*.psd filter=lfs diff=lfs merge=lfs -text\n"));
        assert!(uses_lfs("# art\n  assets/** filter=lfs -text"));
        assert!(!uses_lfs("# *.psd filter=lfs\n*.txt text eol=lf\n"));
        assert!(!uses_lfs("filter=lfs\n"));
        assert!(!uses_lfs(""));
    }

    #[test]
    fn parses_locks() {
        let json = r#"[{"id":"7","path":"art/hero.psd","owner":{"name":"mona"},"locked_at":"2026-10-01T10:00:00Z"},{"id":"8","path":"b.bin"}]"#;
        let locks = parse_lfs_locks(json).unwrap();
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].owner.as_deref(), Some("mona"));
        assert_eq!(locks[1], LfsLock { id: "8".into(), path: "b.bin".into(), owner: None, locked_at: None });
        assert!(parse_lfs_locks("[]\n").unwrap().is_empty());
        assert!(parse_lfs_locks("Remote does not support locking").is_err());
    }

    #[test]
    fn validates_patterns() {
        assert_eq!(validate_lfs_pattern(" *.psd ").unwrap(), "*.psd");
        assert!(validate_lfs_pattern("assets/big file.bin").is_ok());
        for bad in ["", "  ", "#comment", "a\nb", "a\0b"] {
            assert!(validate_lfs_pattern(bad).is_err(), "{bad:?}");
        }
    }
}
