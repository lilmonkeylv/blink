use std::path::Path;
use std::sync::Arc;

use blink_git::Repository;
use dashmap::DashMap;
use tokio::sync::{Semaphore, broadcast};

use crate::error::{Error, Result};
use crate::pools::{PoolConfig, Pools};
use crate::repo_handle::RepoHandle;
use crate::snapshot::RepoSnapshot;

pub type RepoId = String;

#[derive(Debug, Clone)]
pub enum EngineEvent {
    SnapshotChanged { repo: RepoId, generation: u64 },
    RepoClosed { repo: RepoId },
}

pub struct Engine {
    pools: Arc<Pools>,
    repos: DashMap<RepoId, Arc<RepoHandle>>,
    network: Arc<Semaphore>,
    events: broadcast::Sender<EngineEvent>,
}

impl Engine {
    pub fn new(config: PoolConfig) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            network: Arc::new(Semaphore::new(config.network_concurrency.max(1))),
            pools: Arc::new(Pools::new(config)),
            repos: DashMap::new(),
            events,
        }
    }

    pub fn pools(&self) -> &Arc<Pools> {
        &self.pools
    }

    pub fn network(&self) -> &Arc<Semaphore> {
        &self.network
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub async fn open(&self, path: impl AsRef<Path>) -> Result<(RepoId, Arc<RepoHandle>)> {
        let path = path.as_ref().to_path_buf();
        let repo = self.pools.run_foreground(move || Repository::open(path)).await??;
        let id = repo.workdir().to_string_lossy().into_owned();
        let handle = self.repos.entry(id.clone()).or_insert_with(|| Arc::new(RepoHandle::new(repo))).clone();
        Ok((id, handle))
    }

    pub fn get(&self, id: &str) -> Result<Arc<RepoHandle>> {
        self.repos.get(id).map(|r| r.clone()).ok_or_else(|| Error::UnknownRepo(id.to_string()))
    }

    pub fn close(&self, id: &str) {
        if self.repos.remove(id).is_some() {
            let _ = self.events.send(EngineEvent::RepoClosed { repo: id.to_string() });
        }
    }

    pub fn open_repos(&self) -> Vec<RepoId> {
        self.repos.iter().map(|r| r.key().clone()).collect()
    }

    pub async fn refresh(&self, id: &str) -> Result<Arc<RepoSnapshot>> {
        let handle = self.get(id)?;
        let snapshot = handle.refresh(&self.pools).await?;
        let _ =
            self.events.send(EngineEvent::SnapshotChanged { repo: id.to_string(), generation: snapshot.generation });
        Ok(snapshot)
    }

    pub async fn write<F, R>(&self, id: &str, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let handle = self.get(id)?;
        let result = handle.write(&self.pools, op).await;
        match self.refresh(id).await {
            Ok(_) | Err(Error::Cancelled) => {}
            Err(e) => tracing::warn!(repo = id, error = %e, "refresh after write failed"),
        }
        result
    }

    pub async fn rewrite<F, R>(&self, id: &str, label: impl Into<String>, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let handle = self.get(id)?;
        let label = label.into();
        self.write(id, move |repo| handle.undo().record(repo, &label, || op(repo))).await
    }

    pub async fn read<F, R>(&self, id: &str, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        self.get(id)?.read(&self.pools, op).await
    }

    pub async fn network_write<F, R>(&self, id: &str, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let _permit = self.network.clone().acquire_owned().await.map_err(|_| Error::PoolClosed)?;
        self.write(id, op).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blink_git::GitCli;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        GitCli::init(dir.path(), "main").unwrap();
        let cli = GitCli::new(dir.path());
        cli.run(&["config", "user.name", "T"]).unwrap();
        cli.run(&["config", "user.email", "t@example.com"]).unwrap();
        cli.run(&["config", "commit.gpgsign", "false"]).unwrap();
        std::fs::write(dir.path().join("f"), "1\n").unwrap();
        cli.stage_all().unwrap();
        cli.commit("init", false).unwrap();
        dir
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_refresh_write_cycle() {
        let dir = fixture();
        let engine = Engine::new(PoolConfig::default());
        let (id, handle) = engine.open(dir.path()).await.unwrap();
        assert_eq!(handle.snapshot().generation, 0);

        let snap = engine.refresh(&id).await.unwrap();
        assert!(snap.status.files.is_empty());
        assert!(snap.generation > 0);

        std::fs::write(dir.path().join("f"), "2\n").unwrap();
        let snap = engine.refresh(&id).await.unwrap();
        assert_eq!(snap.status.files.len(), 1);

        engine
            .write(&id, |repo| {
                repo.cli().stage_all()?;
                repo.cli().commit("two", false)
            })
            .await
            .unwrap();
        assert!(handle.snapshot().status.files.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_refreshes_publish_monotonic_generations() {
        let dir = fixture();
        let engine = Arc::new(Engine::new(PoolConfig::default()));
        let (id, handle) = engine.open(dir.path()).await.unwrap();
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let engine = engine.clone();
                let id = id.clone();
                tokio::spawn(async move { engine.refresh(&id).await })
            })
            .collect();
        for t in tasks {
            let _ = t.await.unwrap();
        }
        assert!(handle.snapshot().generation >= 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn writes_are_serialized() {
        let dir = fixture();
        let engine = Arc::new(Engine::new(PoolConfig::default()));
        let (id, _) = engine.open(dir.path()).await.unwrap();
        let path = dir.path().to_path_buf();
        let tasks: Vec<_> = (0..6)
            .map(|i| {
                let engine = engine.clone();
                let id = id.clone();
                let path = path.clone();
                tokio::spawn(async move {
                    engine
                        .write(&id, move |repo| {
                            std::fs::write(path.join(format!("file{i}")), "x")?;
                            repo.cli().stage(&[&format!("file{i}")])?;
                            repo.cli().commit(&format!("c{i}"), false)
                        })
                        .await
                })
            })
            .collect();
        for t in tasks {
            t.await.unwrap().unwrap();
        }
        let log = engine.read(&id, |repo| repo.log(None, 0, 100)).await.unwrap();
        assert_eq!(log.len(), 7);
    }
}
