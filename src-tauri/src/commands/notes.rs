use semver::Version;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
const LAST_SEEN_KEY: &str = "lastSeenVersion";
const MAX_ENTRIES: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseNote {
    pub version: String,
    pub date: Option<String>,
    pub body: String,
}

pub fn parse_changelog(text: &str) -> Vec<ReleaseNote> {
    let mut notes: Vec<ReleaseNote> = Vec::new();
    let mut current: Option<ReleaseNote> = None;
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## [") {
            notes.extend(current.take());
            let Some((version, rest)) = heading.split_once(']') else { continue };
            if Version::parse(version).is_err() {
                continue;
            }
            let date = rest.trim().strip_prefix('-').map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
            current = Some(ReleaseNote { version: version.to_string(), date, body: String::new() });
        } else if let Some(note) = current.as_mut() {
            note.body.push_str(line);
            note.body.push('\n');
        }
    }
    notes.extend(current);
    for note in &mut notes {
        note.body = note.body.trim().to_string();
    }
    notes
}

fn notes_between(notes: Vec<ReleaseNote>, after: Option<&Version>, upto: &Version) -> Vec<ReleaseNote> {
    let mut picked: Vec<(Version, ReleaseNote)> = notes
        .into_iter()
        .filter_map(|n| Version::parse(&n.version).ok().map(|v| (v, n)))
        .filter(|(v, _)| v <= upto && after.is_none_or(|a| v > a))
        .collect();
    picked.sort_by(|a, b| b.0.cmp(&a.0));
    picked.into_iter().take(MAX_ENTRIES).map(|(_, n)| n).collect()
}

fn current_version(app: &AppHandle) -> Version {
    let v = &app.package_info().version;
    Version::parse(&v.to_string()).unwrap_or_else(|_| Version::new(0, 0, 0))
}

#[tauri::command]
pub fn whats_new(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Vec<ReleaseNote>> {
    let current = current_version(&app);
    let last_seen: Option<String> = state.cache.app_state(LAST_SEEN_KEY)?;
    state.cache.put_app_state(LAST_SEEN_KEY, &current.to_string())?;
    let Some(last_seen) = last_seen.and_then(|v| Version::parse(&v).ok()) else { return Ok(Vec::new()) };
    if last_seen >= current {
        return Ok(Vec::new());
    }
    Ok(notes_between(parse_changelog(CHANGELOG), Some(&last_seen), &current))
}

#[tauri::command]
pub fn release_notes(app: AppHandle) -> Vec<ReleaseNote> {
    notes_between(parse_changelog(CHANGELOG), None, &current_version(&app))
}

#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> CmdResult<()> {
    let parsed = url::Url::parse(&url).map_err(|e| CommandError::new("url", e.to_string()))?;
    if parsed.scheme() != "https" {
        return Err(CommandError::new("url", "only https links can be opened"));
    }
    app.opener().open_url(parsed.as_str(), None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\nIntro.\n\n## [Unreleased]\n\n### Features\n\n- wip\n\n## [0.3.0] - 2026-11-01\n\n### Fixes\n\n- c\n\n## [0.2.0] - 2026-10-20\n\n### Features\n\n- b\n\n## [0.1.0] - 2026-10-13\n\n- a\n";

    #[test]
    fn parses_versions_and_skips_unreleased() {
        let notes = parse_changelog(SAMPLE);
        let versions: Vec<_> = notes.iter().map(|n| n.version.as_str()).collect();
        assert_eq!(versions, ["0.3.0", "0.2.0", "0.1.0"]);
        assert_eq!(notes[1].date.as_deref(), Some("2026-10-20"));
        assert_eq!(notes[1].body, "### Features\n\n- b");
    }

    #[test]
    fn picks_versions_newer_than_last_seen() {
        let notes = parse_changelog(SAMPLE);
        let picked = notes_between(notes, Some(&Version::new(0, 1, 0)), &Version::new(0, 3, 0));
        let versions: Vec<_> = picked.iter().map(|n| n.version.as_str()).collect();
        assert_eq!(versions, ["0.3.0", "0.2.0"]);
    }

    #[test]
    fn ignores_notes_newer_than_the_running_app() {
        let picked = notes_between(parse_changelog(SAMPLE), None, &Version::new(0, 2, 0));
        assert_eq!(picked.first().map(|n| n.version.as_str()), Some("0.2.0"));
    }
}
