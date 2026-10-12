use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::error::Result;
use crate::log::CommitSummary;

const LOG_FORMAT: &str = "--format=%H%x00%h%x00%s%x00%an%x00%ae%x00%at%x00%P%x1e";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameCommit {
    pub author: String,
    pub time: i64,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub sha: String,
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Blame {
    pub lines: Vec<BlameLine>,
    pub commits: HashMap<String, BlameCommit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchKind {
    Message,
    Author,
    Content,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflogEntry {
    pub sha: String,
    pub selector: String,
    pub subject: String,
    pub time: i64,
}

pub(crate) fn parse_log(output: &str) -> Vec<CommitSummary> {
    output
        .split('\x1e')
        .filter_map(|record| {
            let record = record.trim_start_matches('\n');
            if record.is_empty() {
                return None;
            }
            let mut f = record.split('\0');
            Some(CommitSummary {
                id: f.next()?.to_string(),
                short_id: f.next()?.to_string(),
                summary: f.next()?.to_string(),
                author_name: f.next()?.to_string(),
                author_email: f.next()?.to_string(),
                time: f.next()?.parse().unwrap_or_default(),
                parents: f.next().unwrap_or_default().split_whitespace().map(str::to_string).collect(),
            })
        })
        .collect()
}

pub(crate) fn parse_blame(output: &str) -> Blame {
    let mut blame = Blame::default();
    let mut sha = String::new();
    let mut line_no = 0u32;
    let mut pending: Option<(String, BlameCommit)> = None;

    for line in output.lines() {
        if let Some(text) = line.strip_prefix('\t') {
            if let Some((id, commit)) = pending.take() {
                blame.commits.entry(id).or_insert(commit);
            }
            blame.lines.push(BlameLine { sha: sha.clone(), line: line_no, text: text.to_string() });
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        if key.len() == 40 && key.bytes().all(|b| b.is_ascii_hexdigit()) {
            sha = key.to_string();
            line_no = value.split(' ').nth(1).and_then(|n| n.parse().ok()).unwrap_or(line_no + 1);
            if !blame.commits.contains_key(&sha) {
                pending = Some((sha.clone(), BlameCommit { author: String::new(), time: 0, summary: String::new() }));
            }
            continue;
        }
        if let Some((_, commit)) = pending.as_mut() {
            match key {
                "author" => commit.author = value.to_string(),
                "author-time" => commit.time = value.parse().unwrap_or_default(),
                "summary" => commit.summary = value.to_string(),
                _ => {}
            }
        }
    }
    blame
}

impl GitCli {
    pub fn blame(&self, path: &str, rev: Option<&str>) -> Result<Blame> {
        let mut args = vec!["blame", "--porcelain"];
        if let Some(rev) = rev {
            args.push(rev);
        }
        args.extend(["--", path]);
        Ok(parse_blame(&self.run(&args)?))
    }

    pub fn file_history(&self, path: &str, offset: usize, limit: usize) -> Result<Vec<CommitSummary>> {
        let skip = format!("--skip={offset}");
        let max = format!("-n{limit}");
        Ok(parse_log(&self.run(&["log", "--follow", LOG_FORMAT, &skip, &max, "--", path])?))
    }

    pub fn search(&self, kind: SearchKind, query: &str, limit: usize) -> Result<Vec<CommitSummary>> {
        let max = format!("-n{limit}");
        let mut args = vec!["log", LOG_FORMAT, max.as_str(), "-i"];
        let pattern;
        match kind {
            SearchKind::Message => {
                pattern = format!("--grep={query}");
                args.extend(["--fixed-strings", pattern.as_str()]);
            }
            SearchKind::Author => {
                pattern = format!("--author={query}");
                args.extend(["--fixed-strings", pattern.as_str()]);
            }
            SearchKind::Content => {
                pattern = format!("-S{query}");
                args.push(pattern.as_str());
            }
            SearchKind::Path => {
                pattern = format!("*{query}*");
                args.extend(["--", pattern.as_str()]);
            }
        }
        let mut results = parse_log(&self.run(&args)?);
        if kind == SearchKind::Message
            && query.len() >= 4
            && query.bytes().all(|b| b.is_ascii_hexdigit())
            && let Ok(sha) = self.rev_parse(query)
            && !results.iter().any(|c| c.id == sha)
        {
            let exact = parse_log(&self.run(&["log", LOG_FORMAT, "-n1", &sha])?);
            results.splice(0..0, exact);
        }
        Ok(results)
    }

    pub fn reflog(&self, limit: usize) -> Result<Vec<ReflogEntry>> {
        let max = format!("-n{limit}");
        Ok(self
            .run(&["reflog", "--format=%H%x00%gd%x00%gs%x00%ct", &max])?
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\0');
                Some(ReflogEntry {
                    sha: f.next()?.to_string(),
                    selector: f.next()?.to_string(),
                    subject: f.next()?.to_string(),
                    time: f.next()?.parse().unwrap_or_default(),
                })
            })
            .collect())
    }

    pub fn range_log(&self, from: &str, to: &str, limit: usize) -> Result<Vec<CommitSummary>> {
        let range = format!("{from}..{to}");
        let max = format!("-n{limit}");
        Ok(parse_log(&self.run(&["log", LOG_FORMAT, &max, &range])?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blame_porcelain() {
        let sha_a = "a".repeat(40);
        let sha_b = "b".repeat(40);
        let out = format!(
            "{sha_a} 1 1 2\nauthor Ann\nauthor-time 100\nsummary first\nfilename f\n\tone\n{sha_a} 2 2\n\ttwo\n{sha_b} 3 3 1\nauthor Bob\nauthor-time 200\nsummary second\nfilename f\n\tthree\n"
        );
        let blame = parse_blame(&out);
        assert_eq!(blame.lines.len(), 3);
        assert_eq!(blame.lines[1].sha, sha_a);
        assert_eq!(blame.lines[2].line, 3);
        assert_eq!(blame.commits[&sha_b].author, "Bob");
        assert_eq!(blame.commits[&sha_a].summary, "first");
    }

    #[test]
    fn parses_log_records() {
        let out = "abc\0ab\0subject\0Ann\0a@x\x0042\0p1 p2\x1e\ndef\0de\0root\0Bob\0b@x\x0041\0\x1e\n";
        let log = parse_log(out);
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].parents, ["p1", "p2"]);
        assert!(log[1].parents.is_empty());
        assert_eq!(log[1].time, 41);
    }
}
