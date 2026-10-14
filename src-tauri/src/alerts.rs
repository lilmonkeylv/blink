use blink_store::Notifications;
use chrono::Timelike;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alert {
    ChecksFailed,
    ReviewRequested,
    PrMerged,
    PushRejected,
}

pub fn wanted(n: &Notifications, alert: Alert, hour: u8) -> bool {
    let kind = match alert {
        Alert::ChecksFailed => n.checks_failed,
        Alert::ReviewRequested => n.review_requested,
        Alert::PrMerged => n.pr_merged,
        Alert::PushRejected => n.push_rejected,
    };
    kind && n.allows(hour)
}

pub fn subscribed(app: &AppHandle, alert: Alert) -> bool {
    let hour = chrono::Local::now().hour() as u8;
    wanted(&app.state::<AppState>().settings.read().notifications, alert, hour)
}

pub fn notify(app: &AppHandle, alert: Alert, title: impl Into<String>, body: impl Into<String>) {
    if !subscribed(app, alert) {
        return;
    }
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::debug!(error = %e, "notification failed");
    }
}

pub fn window_focused(app: &AppHandle) -> bool {
    app.webview_windows().values().any(|w| w.is_focused().unwrap_or(false))
}

pub fn is_push_rejection(message: &str) -> bool {
    message.contains("[rejected]") || message.contains("[remote rejected]") || message.contains("non-fast-forward")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_alert_follows_its_own_switch_and_quiet_hours() {
        let mut n = Notifications::default();
        assert!(wanted(&n, Alert::ChecksFailed, 12));
        assert!(wanted(&n, Alert::ReviewRequested, 12));
        assert!(!wanted(&n, Alert::PrMerged, 12));
        assert!(wanted(&n, Alert::PushRejected, 12));
        n.pr_merged = true;
        n.quiet_hours = Some((22, 8));
        assert!(wanted(&n, Alert::PrMerged, 12));
        assert!(!wanted(&n, Alert::PrMerged, 23));
        n.enabled = false;
        assert!(!wanted(&n, Alert::ChecksFailed, 12));
    }

    #[test]
    fn recognises_rejected_pushes() {
        assert!(is_push_rejection(" ! [rejected]        main -> main (fetch first)"));
        assert!(is_push_rejection(" ! [remote rejected] main -> main (protected branch hook declined)"));
        assert!(is_push_rejection(
            "Updates were rejected because the tip of your current branch is behind (non-fast-forward)"
        ));
        assert!(!is_push_rejection("fatal: unable to access 'https://github.com/a/b.git/': Could not resolve host"));
    }
}
