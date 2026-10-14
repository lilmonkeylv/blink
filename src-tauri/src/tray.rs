use blink_store::RepoRecord;
use chrono::Timelike;
use serde::Serialize;
use tauri::menu::{Menu, MenuBuilder, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;

const TRAY_ID: &str = "blink";
const RECENT_LIMIT: usize = 5;
const OPEN_PREFIX: &str = "open:";

pub const TRAY_ACTION: &str = "tray-action";

#[derive(Debug, Clone, PartialEq, Eq)]
enum TrayAction {
    Show,
    FetchAll,
    CheckForUpdates,
    Quit,
    Open(String),
}

impl TrayAction {
    fn parse(id: &str) -> Option<Self> {
        match id {
            "show" => Some(Self::Show),
            "fetch-all" => Some(Self::FetchAll),
            "check-updates" => Some(Self::CheckForUpdates),
            "quit" => Some(Self::Quit),
            other => other.strip_prefix(OPEN_PREFIX).filter(|p| !p.is_empty()).map(|p| Self::Open(p.to_string())),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
enum UiAction {
    FetchAll,
    CheckForUpdates,
}

fn recent_repositories(mut repos: Vec<RepoRecord>, limit: usize) -> Vec<RepoRecord> {
    repos.retain(|r| r.last_opened > 0);
    repos.sort_by_key(|r| std::cmp::Reverse(r.last_opened));
    repos.truncate(limit);
    repos
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let repos = app.state::<AppState>().cache.repositories().unwrap_or_default();
    let recent = recent_repositories(repos, RECENT_LIMIT);
    let mut menu =
        MenuBuilder::new(app).text("show", "Show Blink").separator().text("fetch-all", "Fetch all repositories");
    if !recent.is_empty() {
        let submenu = recent
            .iter()
            .fold(SubmenuBuilder::new(app, "Open recent"), |sub, r| {
                sub.text(format!("{OPEN_PREFIX}{}", r.path), &r.name)
            })
            .build()?;
        menu = menu.item(&submenu);
    }
    menu.separator().text("check-updates", "Check for updates").separator().text("quit", "Quit Blink").build()
}

pub fn keeps_running(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.read().advanced.close_to_tray && app.tray_by_id(TRAY_ID).is_some()
}

pub fn sync(app: &AppHandle) {
    let enabled = app.state::<AppState>().settings.read().advanced.close_to_tray;
    if !enabled {
        app.remove_tray_by_id(TRAY_ID);
        return;
    }
    let menu = match build_menu(app) {
        Ok(menu) => menu,
        Err(e) => {
            tracing::warn!(error = %e, "tray menu unavailable");
            return;
        }
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_menu(Some(menu));
        return;
    }
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Blink")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    if let Err(e) = builder.build(app) {
        tracing::warn!(error = %e, "tray icon unavailable");
    }
}

fn handle(app: &AppHandle, id: &str) {
    let Some(action) = TrayAction::parse(id) else { return };
    match action {
        TrayAction::Show => show_main(app),
        TrayAction::Quit => app.exit(0),
        TrayAction::Open(path) => {
            show_main(app);
            crate::launch::open_path(app, path);
        }
        TrayAction::CheckForUpdates => {
            show_main(app);
            let _ = app.emit_to("main", TRAY_ACTION, UiAction::CheckForUpdates);
        }
        TrayAction::FetchAll if main_visible(app) => {
            let _ = app.emit_to("main", TRAY_ACTION, UiAction::FetchAll);
        }
        TrayAction::FetchAll => fetch_all_in_background(app.clone()),
    }
}

fn fetch_summary(fetched: usize, behind: usize, failed: usize) -> String {
    let plural = |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { format!("{n} {many}") };
    let mut parts = vec![format!("Fetched {}.", plural(fetched, "1 repository", "repositories"))];
    if behind > 0 {
        parts.push(format!("{} new commits to pull.", plural(behind, "1 has", "have")));
    }
    if failed > 0 {
        parts.push(format!("{failed} failed."));
    }
    parts.join(" ")
}

fn fetch_all_in_background(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let rows = match crate::commands::sync::fetch_all(app.clone(), app.state()).await {
            Ok(rows) => rows,
            Err(e) => {
                tracing::debug!(error = %e.message, "tray fetch failed");
                return;
            }
        };
        let hour = chrono::Local::now().hour() as u8;
        let notify = app.state::<AppState>().settings.read().notifications.allows(hour);
        if !notify {
            return;
        }
        let failed = rows.iter().filter(|r| r.error.is_some()).count();
        let behind = rows.iter().filter(|r| r.behind > 0).count();
        let _ = app
            .notification()
            .builder()
            .title("Fetch complete")
            .body(fetch_summary(rows.len() - failed, behind, failed))
            .show();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, last_opened: i64) -> RepoRecord {
        RepoRecord { path: path.into(), name: path.into(), owner: None, remote_url: None, last_opened }
    }

    #[test]
    fn menu_ids_map_to_actions() {
        assert_eq!(TrayAction::parse("show"), Some(TrayAction::Show));
        assert_eq!(TrayAction::parse("fetch-all"), Some(TrayAction::FetchAll));
        assert_eq!(TrayAction::parse("check-updates"), Some(TrayAction::CheckForUpdates));
        assert_eq!(TrayAction::parse("quit"), Some(TrayAction::Quit));
        assert_eq!(TrayAction::parse(r"open:C:\dev\my repo"), Some(TrayAction::Open(r"C:\dev\my repo".into())));
        assert_eq!(TrayAction::parse("open:"), None);
        assert_eq!(TrayAction::parse("unknown"), None);
    }

    #[test]
    fn recent_repositories_are_newest_first_and_skip_unopened() {
        let repos = vec![record("a", 10), record("b", 0), record("c", 30), record("d", 20)];
        let names: Vec<_> = recent_repositories(repos.clone(), 5).into_iter().map(|r| r.path).collect();
        assert_eq!(names, ["c", "d", "a"]);
        assert_eq!(recent_repositories(repos, 2).len(), 2);
    }

    #[test]
    fn fetch_summaries_read_naturally() {
        assert_eq!(fetch_summary(1, 0, 0), "Fetched 1 repository.");
        assert_eq!(fetch_summary(4, 1, 0), "Fetched 4 repositories. 1 has new commits to pull.");
        assert_eq!(fetch_summary(3, 2, 2), "Fetched 3 repositories. 2 have new commits to pull. 2 failed.");
    }
}
