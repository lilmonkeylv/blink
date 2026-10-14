use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};

const PENDING: &str = "last-crash.txt";

pub fn install(log_dir: &Path) {
    let pending = log_dir.join(PENDING);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = report(info, &std::backtrace::Backtrace::force_capture().to_string());
        let _ = std::fs::create_dir_all(pending.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(&pending, &report);
        tracing::error!("{report}");
        previous(info);
    }));
}

fn report(info: &PanicHookInfo<'_>, backtrace: &str) -> String {
    let text = format!(
        "Blink {} ({} {})\n{info}\n\n{backtrace}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    blink_git::redact(&text)
}

pub fn take_pending(log_dir: &Path) -> Option<PathBuf> {
    let pending = log_dir.join(PENDING);
    if !pending.is_file() {
        return None;
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let kept = log_dir.join(format!("crash-{stamp}.txt"));
    std::fs::rename(&pending, &kept).ok()?;
    Some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_report_is_kept_once() {
        let dir = tempfile::tempdir().unwrap();
        assert!(take_pending(dir.path()).is_none());
        std::fs::write(dir.path().join(PENDING), "boom").unwrap();
        let kept = take_pending(dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "boom");
        assert!(take_pending(dir.path()).is_none());
    }
}
