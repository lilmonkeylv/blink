use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use arc_swap::ArcSwap;
use blink_git::Repository;
use parking_lot::Mutex;

use crate::cancel::Cancel;
use crate::error::{Error, Result};
use crate::pools::Pools;
use crate::snapshot::RepoSnapshot;
use crate::undo::UndoJournal;

pub struct RepoHandle {
    repo: Repository,
    snapshot: ArcSwap<RepoSnapshot>,
    generation: AtomicU64,
    writer: tokio::sync::Mutex<()>,
    inflight_refresh: Mutex<Option<Cancel>>,
    undo: UndoJournal,
}

impl RepoHandle {
    pub fn new(repo: Repository) -> Self {
        Self {
            repo,
            snapshot: ArcSwap::from_pointee(RepoSnapshot::empty()),
            generation: AtomicU64::new(0),
            writer: tokio::sync::Mutex::new(()),
            inflight_refresh: Mutex::new(None),
            undo: UndoJournal::default(),
        }
    }

    pub fn repo(&self) -> &Repository {
        &self.repo
    }

    pub fn undo(&self) -> &UndoJournal {
        &self.undo
    }

    pub fn snapshot(&self) -> Arc<RepoSnapshot> {
        self.snapshot.load_full()
    }

    pub async fn refresh(self: &Arc<Self>, pools: &Pools) -> Result<Arc<RepoSnapshot>> {
        let cancel = Cancel::new();
        if let Some(previous) = self.inflight_refresh.lock().replace(cancel.clone()) {
            previous.cancel();
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let this = self.clone();
        let flag = cancel.flag();

        let computed = pools
            .run_foreground(move || -> Result<RepoSnapshot> {
                let repo = &this.repo;
                let (status, (head, ahead_behind)) = rayon::join(
                    || repo.status(Some(flag)),
                    || (repo.head().ok(), repo.current_ahead_behind().ok().flatten()),
                );
                Ok(RepoSnapshot {
                    generation,
                    head,
                    status: status?,
                    ahead_behind,
                    operation: repo.is_operation_in_progress(),
                    has_remote: repo.has_remotes(),
                    taken_at: Instant::now(),
                })
            })
            .await?;

        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let snapshot = Arc::new(computed?);
        let mut published = false;
        self.snapshot.rcu(|current| {
            if current.generation < snapshot.generation {
                published = true;
                snapshot.clone()
            } else {
                published = false;
                current.clone()
            }
        });
        if !published {
            return Err(Error::Cancelled);
        }
        Ok(snapshot)
    }

    pub async fn write<F, R>(self: &Arc<Self>, pools: &Pools, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let _guard = self.writer.lock().await;
        let this = self.clone();
        let result = pools.run_foreground(move || op(&this.repo)).await??;
        Ok(result)
    }

    pub async fn read<F, R>(self: &Arc<Self>, pools: &Pools, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let this = self.clone();
        Ok(pools.run_foreground(move || op(&this.repo)).await??)
    }
}
