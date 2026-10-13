use std::io::{self, Write};
use std::path::Path;

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init(log_dir: Option<&Path>) {
    let filter = tracing_subscriber::EnvFilter::try_from_env("BLINK_LOG").unwrap_or_else(|_| "info,blink=debug".into());
    let file = log_dir.and_then(|dir| {
        std::fs::create_dir_all(dir).ok()?;
        std::fs::OpenOptions::new().create(true).append(true).open(dir.join("blink.log")).ok()
    });
    let file_layer =
        file.map(|f| tracing_subscriber::fmt::layer().with_ansi(false).with_writer(Redacted(std::sync::Mutex::new(f))));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(Redacted(io::stdout)))
        .with(file_layer)
        .init();
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
}
