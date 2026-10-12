use std::future::Future;

use tokio::sync::oneshot;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    pub foreground_threads: usize,
    pub background_threads: usize,
    pub network_concurrency: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        Self {
            foreground_threads: cores.saturating_sub(1).max(1),
            background_threads: (cores / 4).max(1),
            network_concurrency: 4,
        }
    }
}

pub struct Pools {
    foreground: rayon::ThreadPool,
    background: rayon::ThreadPool,
    config: PoolConfig,
}

impl Pools {
    pub fn new(config: PoolConfig) -> Self {
        let foreground = rayon::ThreadPoolBuilder::new()
            .num_threads(config.foreground_threads)
            .thread_name(|i| format!("blink-fg-{i}"))
            .build()
            .expect("foreground pool");
        let background = rayon::ThreadPoolBuilder::new()
            .num_threads(config.background_threads)
            .thread_name(|i| format!("blink-bg-{i}"))
            .start_handler(|_| lower_current_thread_priority())
            .build()
            .expect("background pool");
        Self { foreground, background, config }
    }

    pub fn config(&self) -> PoolConfig {
        self.config
    }

    pub fn foreground(&self) -> &rayon::ThreadPool {
        &self.foreground
    }

    pub fn background(&self) -> &rayon::ThreadPool {
        &self.background
    }

    pub fn run_foreground<F, R>(&self, job: F) -> impl Future<Output = Result<R>> + use<F, R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        spawn_on(&self.foreground, job)
    }

    pub fn run_background<F, R>(&self, job: F) -> impl Future<Output = Result<R>> + use<F, R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        spawn_on(&self.background, job)
    }
}

fn spawn_on<F, R>(pool: &rayon::ThreadPool, job: F) -> impl Future<Output = Result<R>> + use<F, R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    pool.spawn(move || {
        let _ = tx.send(job());
    });
    async move { rx.await.map_err(|_| Error::PoolClosed) }
}

#[cfg(windows)]
fn lower_current_thread_priority() {
    unsafe extern "system" {
        fn GetCurrentThread() -> isize;
        fn SetThreadPriority(thread: isize, priority: i32) -> i32;
    }
    const THREAD_MODE_BACKGROUND_BEGIN: i32 = 0x0001_0000;
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

#[cfg(unix)]
fn lower_current_thread_priority() {
    unsafe extern "C" {
        fn nice(inc: i32) -> i32;
    }
    // nice() is per-thread on Linux but process-wide on macOS.
    if cfg!(target_os = "linux") {
        unsafe {
            nice(10);
        }
    }
}

#[cfg(not(any(windows, unix)))]
fn lower_current_thread_priority() {}
