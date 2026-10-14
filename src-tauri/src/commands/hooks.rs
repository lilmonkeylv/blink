use blink_git::HookEvent;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const HOOK_OUTPUT: &str = "hook-output";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HookAction {
    Commit,
    Push,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookPayload {
    repo: String,
    action: HookAction,
    event: HookEvent,
}

pub(crate) fn hook_emitter(app: &AppHandle, repo: &str, action: HookAction) -> impl FnMut(HookEvent) + Send + 'static {
    let app = app.clone();
    let repo = repo.to_string();
    move |event| {
        let _ = app.emit(HOOK_OUTPUT, HookPayload { repo: repo.clone(), action, event });
    }
}
