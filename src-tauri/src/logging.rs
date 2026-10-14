use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, reload};

const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;
const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

static FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

pub fn init(log_dir: Option<&Path>, level: &str) {
    let filter = EnvFilter::try_from_env("BLINK_LOG").unwrap_or_else(|_| directive(level).into());
    let (filter, handle) = reload::Layer::new(filter);
    if std::env::var_os("BLINK_LOG").is_none() {
        let _ = FILTER.set(handle);
    }
    let file = log_dir.and_then(|dir| {
        std::fs::create_dir_all(dir).ok()?;
        RotatingFile::open(dir.join("blink.log"), MAX_LOG_BYTES).ok()
    });
    let file_layer =
        file.map(|f| tracing_subscriber::fmt::layer().with_ansi(false).with_writer(Redacted(std::sync::Mutex::new(f))));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(Redacted(io::stdout)))
        .with(file_layer)
        .init();
}

pub fn set_level(level: &str) {
    if let Some(handle) = FILTER.get()
        && let Err(e) = handle.reload(EnvFilter::new(directive(level)))
    {
        tracing::warn!(error = %e, "could not change the log level");
    }
}

fn directive(level: &str) -> String {
    let level = level.trim().to_ascii_lowercase();
    let level = if LEVELS.contains(&level.as_str()) { level } else { "info".to_string() };
    let dependencies = if matches!(level.as_str(), "error" | "warn") { level.as_str() } else { "info" };
    format!("{dependencies},blink={level}")
}

pub struct RotatingFile {
    path: PathBuf,
    file: File,
    written: u64,
    limit: u64,
}

impl RotatingFile {
    pub fn open(path: PathBuf, limit: u64) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata()?.len();
        let mut log = Self { path, file, written, limit };
        if log.written >= limit {
            log.rotate()?;
        }
        Ok(log)
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        std::fs::rename(&self.path, self.path.with_extension("log.1"))?;
        self.file = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        self.written = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written > 0 && self.written + buf.len() as u64 > self.limit && self.rotate().is_err() {
            self.written = 0;
        }
        let n = self.file.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub struct Redacted<M>(pub M);

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for Redacted<M> {
    type Writer = RedactedWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactedWriter(self.0.make_writer())
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        RedactedWriter(self.0.make_writer_for(meta))
    }
}

pub struct RedactedWriter<W>(W);

impl<W: Write> Write for RedactedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_all(blink_git::redact(&String::from_utf8_lossy(buf)).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl Write for Capture {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn formatter_output_is_redacted() {
        let capture = Capture::default();
        let sink = capture.clone();
        let subscriber =
            tracing_subscriber::fmt().with_ansi(false).with_writer(Redacted(move || sink.clone())).finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(
                remote = "https://x-access-token:ghs_16C7e42F292c6912E7710c838347Ae178B4a@github.com/acme/app.git",
                "fetch failed"
            );
            tracing::warn!(error = "fatal: https://alice:hunter2@example.com/repo", "push failed");
            tracing::info!(header = "Authorization: Bearer gho_16C7e42F292c6912E7710c838347Ae178B4a", "request");
            tracing::info!("token github_pat_11ABCDEFG0123456789_abcdefghijklmnopqrstuvwxyz rejected");
        });
        let output = String::from_utf8(capture.0.lock().clone()).unwrap();
        for secret in ["ghs_16C7", "hunter2", "alice", "gho_16C7", "github_pat_11ABC", "x-access-token"] {
            assert!(!output.contains(secret), "{secret} leaked into {output}");
        }
        assert!(output.contains("https://***@github.com/acme/app.git"));
        assert!(output.contains("Authorization: ***"));
        assert!(output.contains("github_pat_*** rejected"));
    }

    #[test]
    fn log_level_setting_maps_to_a_filter() {
        assert_eq!(directive("debug"), "info,blink=debug");
        assert_eq!(directive(" TRACE "), "info,blink=trace");
        assert_eq!(directive("warn"), "warn,blink=warn");
        assert_eq!(directive("error"), "error,blink=error");
        assert_eq!(directive("loud"), "info,blink=info");
        assert!(EnvFilter::try_new(directive("debug")).is_ok());
    }

    #[test]
    fn rotates_past_the_limit_keeping_one_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blink.log");
        let old = dir.path().join("blink.log.1");
        let mut log = RotatingFile::open(path.clone(), 10).unwrap();
        log.write_all(b"12345678").unwrap();
        log.write_all(b"abcdef").unwrap();
        assert_eq!(std::fs::read(&old).unwrap(), b"12345678");
        assert_eq!(std::fs::read(&path).unwrap(), b"abcdef");
        log.write_all(b"ghijklmn").unwrap();
        assert_eq!(std::fs::read(&old).unwrap(), b"abcdef");
        assert_eq!(std::fs::read(&path).unwrap(), b"ghijklmn");
        drop(log);

        std::fs::write(&path, b"0123456789AB").unwrap();
        let log = RotatingFile::open(path.clone(), 10).unwrap();
        assert_eq!(log.written, 0);
        assert_eq!(std::fs::read(&old).unwrap(), b"0123456789AB");
        assert!(std::fs::read(&path).unwrap().is_empty());
    }
}
