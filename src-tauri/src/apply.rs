use std::path::{Path, PathBuf};
use std::time::Duration;

use blink_store::{GitExecutable, Settings};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

const MB: u64 = 1024 * 1024;
const PRUNE_EVERY: Duration = Duration::from_secs(30 * 60);

pub fn apply_settings(app: &AppHandle, previous: Option<&Settings>, settings: &Settings) {
    let exe_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
    let system_root = std::env::var_os("SystemRoot").map(PathBuf::from);
    blink_git::configure(git_runtime(settings, exe_dir.as_deref(), system_root.as_deref()));
    crate::logging::set_level(&settings.advanced.log_level);
    if previous.is_some_and(|p| p.advanced.watcher != settings.advanced.watcher) {
        crate::watch::restart_watchers(app, &app.state::<AppState>());
    }
    if previous.is_some_and(|p| p.advanced.cache_limit_mb != settings.advanced.cache_limit_mb) {
        prune_cache(app.clone());
    }
}

pub fn git_runtime(settings: &Settings, exe_dir: Option<&Path>, system_root: Option<&Path>) -> blink_git::Runtime {
    let use_openssh = cfg!(windows) && settings.advanced.use_system_openssh;
    blink_git::Runtime {
        executable: git_executable(settings, exe_dir),
        ssh: if use_openssh { blink_git::system_openssh(system_root) } else { None },
        signing_key: settings.git.signing_key.as_deref().map(str::trim).filter(|k| !k.is_empty()).map(str::to_string),
    }
}

fn git_executable(settings: &Settings, exe_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = settings.git.executable_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    match settings.git.executable {
        GitExecutable::System => None,
        GitExecutable::Bundled => bundled_git(exe_dir?),
    }
}

fn bundled_git(dir: &Path) -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(windows) { &["git/cmd/git.exe", "git/bin/git.exe"] } else { &["git/bin/git"] };
    candidates.iter().map(|c| dir.join(c)).find(|p| p.is_file())
}

pub fn prune_cache(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let limit = u64::from(state.settings.read().advanced.cache_limit_mb) * MB;
        match state.cache.enforce_limit(&crate::commands::images::avatar_dir(&state.paths), limit) {
            Ok(p) if p.bytes > 0 => tracing::info!(rows = p.rows, files = p.files, bytes = p.bytes, "pruned the cache"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "cache pruning failed"),
        }
    });
}

pub fn spawn_cache_pruning(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            prune_cache(app.clone());
            tokio::time::sleep(PRUNE_EVERY).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_key_and_git_path_reach_the_runtime() {
        let mut s = Settings::default();
        assert_eq!(git_runtime(&s, None, None), blink_git::Runtime::default());
        s.git.signing_key = Some("  ".into());
        s.git.executable_path = Some(" ".into());
        assert_eq!(git_runtime(&s, None, None), blink_git::Runtime::default());
        s.git.signing_key = Some(" 3AA5C34371567BD2 ".into());
        s.git.executable_path = Some("C:/Tools/git/cmd/git.exe".into());
        let rt = git_runtime(&s, None, None);
        assert_eq!(rt.signing_key.as_deref(), Some("3AA5C34371567BD2"));
        assert_eq!(rt.executable, Some(PathBuf::from("C:/Tools/git/cmd/git.exe")));
    }

    #[test]
    fn bundled_git_is_found_next_to_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        s.git.executable = GitExecutable::Bundled;
        assert_eq!(git_runtime(&s, Some(dir.path()), None).executable, None);
        let rel = if cfg!(windows) { "git/cmd/git.exe" } else { "git/bin/git" };
        let git = dir.path().join(rel);
        std::fs::create_dir_all(git.parent().unwrap()).unwrap();
        std::fs::write(&git, b"").unwrap();
        assert_eq!(git_runtime(&s, Some(dir.path()), None).executable, Some(git));
        s.git.executable = GitExecutable::System;
        assert_eq!(git_runtime(&s, Some(dir.path()), None).executable, None);
    }

    #[test]
    fn system_openssh_only_when_enabled_and_present() {
        let root = tempfile::tempdir().unwrap();
        let ssh = root.path().join("System32").join("OpenSSH").join("ssh.exe");
        std::fs::create_dir_all(ssh.parent().unwrap()).unwrap();
        std::fs::write(&ssh, b"").unwrap();
        let mut s = Settings::default();
        assert_eq!(git_runtime(&s, None, Some(root.path())).ssh, None);
        s.advanced.use_system_openssh = true;
        let expected = cfg!(windows).then_some(ssh);
        assert_eq!(git_runtime(&s, None, Some(root.path())).ssh, expected);
        assert_eq!(git_runtime(&s, None, Some(&root.path().join("nope"))).ssh, None);
    }
}
