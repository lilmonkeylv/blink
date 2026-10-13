use std::io::Write;

use blink_diff::{DiffOptions, FileDiff, LineSelection, PatchDirection, SubmoduleDiff, build_partial_patch, diff_file};
use blink_git::{BlobSource, ChangeKind, Error, FileChange, Repository, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSelection {
    pub path: String,
    #[serde(default)]
    pub lines: Option<Vec<(usize, usize)>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitMessage {
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub co_authors: Vec<String>,
}

impl CommitMessage {
    pub fn render(&self) -> String {
        let mut message = self.summary.trim().to_string();
        let description = self.description.trim();
        if !description.is_empty() {
            message.push_str("\n\n");
            message.push_str(description);
        }
        if !self.co_authors.is_empty() {
            message.push_str("\n\n");
            for author in &self.co_authors {
                message.push_str(&format!("Co-authored-by: {author}\n"));
            }
        }
        message
    }
}

fn selection_from(lines: &[(usize, usize)]) -> LineSelection {
    let mut selection = LineSelection::new();
    for &(hunk, line) in lines {
        selection.select(hunk, line);
    }
    selection
}

fn read_or_empty(repo: &Repository, source: &BlobSource, path: &str) -> Result<Vec<u8>> {
    Ok(repo.read_blob(source, path)?.unwrap_or_default())
}

fn submodule_cli(repo: &Repository, path: &str) -> Option<blink_git::GitCli> {
    let dir = repo.workdir().join(path);
    dir.join(".git").exists().then(|| repo.cli().relocated(dir))
}

fn submodule_diff(repo: &Repository, path: &str, old: Option<String>, new: Option<String>, dirty: bool) -> FileDiff {
    let commits = match (submodule_cli(repo, path), &old, &new) {
        (Some(cli), Some(old), Some(new)) if old != new => cli
            .run(&["log", "--format=%h%x09%s", "-n", "50", &format!("{old}..{new}")])
            .map(|out| {
                out.lines()
                    .filter_map(|l| l.split_once('\t'))
                    .map(|(sha, summary)| (sha.to_string(), summary.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    FileDiff { submodule: Some(SubmoduleDiff { old, new, dirty, commits }), ..Default::default() }
}

pub fn working_diff(repo: &Repository, change: &FileChange, opts: DiffOptions) -> Result<FileDiff> {
    let old_path = change.old_path.as_deref().unwrap_or(&change.path);
    let old_link = repo.gitlink_at("HEAD", old_path)?;
    if let Some(cli) = submodule_cli(repo, &change.path) {
        let new_link = cli.run(&["rev-parse", "HEAD"]).ok().map(|s| s.trim().to_string());
        let dirty = cli.run(&["status", "--porcelain"]).is_ok_and(|s| !s.trim().is_empty());
        return Ok(submodule_diff(repo, &change.path, old_link, new_link, dirty));
    }
    if old_link.is_some() {
        return Ok(submodule_diff(repo, &change.path, old_link, None, false));
    }
    let old = read_or_empty(repo, &BlobSource::Head, old_path)?;
    let new = read_or_empty(repo, &BlobSource::Worktree, &change.path)?;
    Ok(diff_file(&change.path, &old, &new, opts))
}

pub fn commit_diff(
    repo: &Repository,
    sha: &str,
    path: &str,
    old_path: Option<&str>,
    opts: DiffOptions,
) -> Result<FileDiff> {
    let old_link = repo.gitlink_at(&format!("{sha}^"), old_path.unwrap_or(path))?;
    let new_link = repo.gitlink_at(sha, path)?;
    if old_link.is_some() || new_link.is_some() {
        return Ok(submodule_diff(repo, path, old_link, new_link, false));
    }
    let old = read_or_empty(repo, &BlobSource::Revision(format!("{sha}^")), old_path.unwrap_or(path))?;
    let new = read_or_empty(repo, &BlobSource::Revision(sha.to_string()), path)?;
    Ok(diff_file(path, &old, &new, opts))
}

fn find_change<'a>(changes: &'a [FileChange], path: &str) -> Result<&'a FileChange> {
    changes.iter().find(|c| c.path == path).ok_or_else(|| Error::Git(format!("no change for {path}")))
}

pub fn commit_selection(
    repo: &Repository,
    files: &[FileSelection],
    message: &CommitMessage,
    amend: bool,
    sign: bool,
) -> Result<String> {
    if message.summary.trim().is_empty() {
        return Err(Error::Git("commit summary is required".into()));
    }
    let cli = repo.cli().with_signing(sign);
    let status = repo.status(None)?;
    cli.unstage_all()?;

    let mut whole: Vec<&str> = Vec::new();
    for file in files {
        let change = find_change(&status.files, &file.path)?;
        match &file.lines {
            None => {
                whole.push(&change.path);
                if let Some(old) = &change.old_path {
                    whole.push(old);
                }
            }
            Some(lines) => {
                let diff = working_diff(repo, change, DiffOptions { max_lines: None, ..Default::default() })?;
                let new_file = matches!(change.kind(), ChangeKind::Untracked | ChangeKind::Added);
                if let Some(patch) =
                    build_partial_patch(&change.path, &diff, &selection_from(lines), PatchDirection::Stage, new_file)
                {
                    cli.apply_to_index(&patch)?;
                }
            }
        }
    }
    cli.stage(&whole)?;
    cli.commit(&message.render(), amend)
}

pub fn discard_files(repo: &Repository, changes: &[FileChange]) -> Result<()> {
    let cli = repo.cli();
    let mut restore: Vec<&str> = Vec::new();
    let mut to_trash = Vec::new();
    let mut unstage: Vec<&str> = Vec::new();

    for change in changes {
        match change.kind() {
            ChangeKind::Untracked => to_trash.push(repo.workdir().join(&change.path)),
            ChangeKind::Added => {
                unstage.push(&change.path);
                to_trash.push(repo.workdir().join(&change.path));
            }
            ChangeKind::Renamed | ChangeKind::Copied => {
                unstage.push(&change.path);
                to_trash.push(repo.workdir().join(&change.path));
                if let Some(old) = &change.old_path {
                    restore.push(old);
                }
            }
            _ => restore.push(&change.path),
        }
    }
    cli.unstage(&unstage)?;
    let existing: Vec<_> = to_trash.into_iter().filter(|p| p.exists()).collect();
    if !existing.is_empty() {
        trash::delete_all(&existing).map_err(|e| Error::Git(format!("move to trash failed: {e}")))?;
    }
    cli.discard_tracked(&restore)
}

pub fn discard_lines(repo: &Repository, change: &FileChange, lines: &[(usize, usize)]) -> Result<()> {
    let diff = working_diff(repo, change, DiffOptions { max_lines: None, ..Default::default() })?;
    match build_partial_patch(&change.path, &diff, &selection_from(lines), PatchDirection::Discard, false) {
        Some(patch) => repo.cli().reverse_apply_to_worktree(&patch),
        None => Ok(()),
    }
}

pub fn ignore(repo: &Repository, pattern: &str) -> Result<()> {
    let path = repo.workdir().join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == pattern) {
        return Ok(());
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file)?;
    }
    writeln!(file, "{pattern}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blink_git::GitCli;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        GitCli::init(dir.path(), "main").unwrap();
        let cli = GitCli::new(dir.path());
        for (k, v) in [
            ("user.name", "T"),
            ("user.email", "t@example.com"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            cli.run(&["config", k, v]).unwrap();
        }
        fs::write(dir.path().join("a.txt"), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").unwrap();
        fs::write(dir.path().join("b.txt"), "b\n").unwrap();
        cli.stage_all().unwrap();
        cli.commit("init", false).unwrap();
        let repo = Repository::open(dir.path()).unwrap();
        (dir, repo)
    }

    fn msg(s: &str) -> CommitMessage {
        CommitMessage { summary: s.into(), ..Default::default() }
    }

    fn change(repo: &Repository, path: &str) -> FileChange {
        repo.status(None).unwrap().files.into_iter().find(|f| f.path == path).unwrap()
    }

    #[test]
    fn submodule_bump_lists_new_commits() {
        let (dir, repo) = fixture();
        let (lib, _) = fixture();
        let cli = repo.cli();
        let url = lib.path().to_string_lossy().replace('\\', "/");
        cli.run(&["-c", "protocol.file.allow=always", "submodule", "add", &url, "lib"]).unwrap();
        cli.commit("add lib", false).unwrap();
        let sub = GitCli::new(dir.path().join("lib"));
        for (k, v) in [("user.name", "T"), ("user.email", "t@example.com"), ("commit.gpgsign", "false")] {
            sub.run(&["config", k, v]).unwrap();
        }
        fs::write(dir.path().join("lib/c.txt"), "c\n").unwrap();
        sub.stage_all().unwrap();
        sub.commit("add c", false).unwrap();

        let diff = working_diff(&repo, &change(&repo, "lib"), DiffOptions::default()).unwrap();
        let submodule = diff.submodule.unwrap();
        assert!(submodule.old.is_some() && submodule.new.is_some());
        assert_ne!(submodule.old, submodule.new);
        assert!(!submodule.dirty);
        assert_eq!(submodule.commits.len(), 1);
        assert_eq!(submodule.commits[0].1, "add c");

        let head = cli.rev_parse("HEAD").unwrap();
        let added = commit_diff(&repo, &head, "lib", None, DiffOptions::default()).unwrap().submodule.unwrap();
        assert!(added.old.is_none() && added.new.is_some());
    }

    #[test]
    fn commits_only_selected_files() {
        let (dir, repo) = fixture();
        fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
        fs::write(dir.path().join("b.txt"), "changed\n").unwrap();
        commit_selection(&repo, &[FileSelection { path: "a.txt".into(), lines: None }], &msg("only a"), false, false)
            .unwrap();
        let status = repo.status(None).unwrap();
        assert_eq!(status.files.len(), 1);
        assert_eq!(status.files[0].path, "b.txt");
    }

    #[test]
    fn commits_selected_lines_only() {
        let (dir, repo) = fixture();
        fs::write(dir.path().join("a.txt"), "ONE\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n").unwrap();
        let c = change(&repo, "a.txt");
        let diff = working_diff(&repo, &c, DiffOptions::default()).unwrap();
        assert_eq!(diff.hunks.len(), 2);
        let lines: Vec<(usize, usize)> = diff.hunks[0]
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind != blink_diff::LineKind::Context)
            .map(|(i, _)| (0, i))
            .collect();
        commit_selection(
            &repo,
            &[FileSelection { path: "a.txt".into(), lines: Some(lines) }],
            &msg("first line"),
            false,
            false,
        )
        .unwrap();

        let committed = repo.read_blob(&BlobSource::Head, "a.txt").unwrap().unwrap();
        assert_eq!(committed, b"ONE\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
        let remaining = working_diff(&repo, &change(&repo, "a.txt"), DiffOptions::default()).unwrap();
        assert_eq!(remaining.hunks.len(), 1);
        assert_eq!(remaining.additions, 1);
    }

    #[test]
    fn commits_partial_new_file() {
        let (dir, repo) = fixture();
        fs::write(dir.path().join("new.txt"), "keep\ndrop\n").unwrap();
        let c = change(&repo, "new.txt");
        let diff = working_diff(&repo, &c, DiffOptions::default()).unwrap();
        let keep = diff.hunks[0].lines.iter().position(|l| l.text == "keep").unwrap();
        commit_selection(
            &repo,
            &[FileSelection { path: "new.txt".into(), lines: Some(vec![(0, keep)]) }],
            &msg("partial"),
            false,
            false,
        )
        .unwrap();
        assert_eq!(repo.read_blob(&BlobSource::Head, "new.txt").unwrap().unwrap(), b"keep\n");
    }

    #[test]
    fn discards_selected_lines() {
        let (dir, repo) = fixture();
        fs::write(dir.path().join("a.txt"), "ONE\n2\n3\n4\n5\n6\n7\n8\n9\nTEN\n").unwrap();
        let c = change(&repo, "a.txt");
        let diff = working_diff(&repo, &c, DiffOptions::default()).unwrap();
        let lines: Vec<_> = diff.hunks[1]
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind != blink_diff::LineKind::Context)
            .map(|(i, _)| (1, i))
            .collect();
        discard_lines(&repo, &c, &lines).unwrap();
        assert_eq!(fs::read(dir.path().join("a.txt")).unwrap(), b"ONE\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
    }

    #[test]
    fn discard_restores_tracked_file() {
        let (dir, repo) = fixture();
        fs::write(dir.path().join("b.txt"), "dirty\n").unwrap();
        discard_files(&repo, &[change(&repo, "b.txt")]).unwrap();
        assert!(repo.status(None).unwrap().files.is_empty());
    }

    #[test]
    fn ignore_appends_once() {
        let (dir, repo) = fixture();
        ignore(&repo, "*.log").unwrap();
        ignore(&repo, "*.log").unwrap();
        assert_eq!(fs::read_to_string(dir.path().join(".gitignore")).unwrap(), "*.log\n");
    }

    #[test]
    fn message_with_coauthors() {
        let m = CommitMessage { summary: "s".into(), description: "d".into(), co_authors: vec!["A <a@x>".into()] };
        assert_eq!(m.render(), "s\n\nd\n\nCo-authored-by: A <a@x>\n");
    }
}
