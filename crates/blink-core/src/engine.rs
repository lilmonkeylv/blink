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

    pub async fn open_detached(&self, path: impl AsRef<Path>) -> Result<Arc<RepoHandle>> {
        let path = path.as_ref().to_path_buf();
        let repo = self.pools.run_foreground(move || Repository::open(path)).await??;
        Ok(Arc::new(RepoHandle::new(repo)))
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
        // the running write refreshes when it finishes; a snapshot taken now would show half-written files
        if handle.is_writing() {
            return Ok(handle.snapshot());
        }
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
        self.refresh_after_write(id).await;
        result
    }

    async fn refresh_after_write(&self, id: &str) {
        match self.refresh(id).await {
            Ok(_) | Err(Error::Cancelled) => {}
            Err(e) => tracing::warn!(repo = id, error = %e, "refresh after write failed"),
        }
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
        let handle = self.get(id)?;
        let result = self.network_on(&handle, true, op).await;
        self.refresh_after_write(id).await;
        result
    }

    pub async fn network_rewrite<F, R>(&self, id: &str, label: impl Into<String>, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let handle = self.get(id)?;
        let label = label.into();
        self.network_write(id, move |repo| handle.undo().record(repo, &label, || op(repo))).await
    }

    pub async fn fetch<F, R>(&self, id: &str, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let handle = self.get(id)?;
        let result = self.network_on(&handle, false, op).await;
        self.refresh_after_write(id).await;
        result
    }

    pub async fn fetch_detached<F, R>(&self, handle: &Arc<RepoHandle>, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        self.network_on(handle, false, op).await
    }

    async fn network_on<F, R>(&self, handle: &Arc<RepoHandle>, exclusive: bool, op: F) -> Result<R>
    where
        F: FnOnce(&Repository) -> blink_git::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let _permit = self.network.clone().acquire_owned().await.map_err(|_| Error::PoolClosed)?;
        handle.network(&self.pools, exclusive, op).await
    }

    pub async fn maintain_commit_graph(&self, id: &str) -> Result<bool> {
        let repo = self.get(id)?.repo().clone();
        let id = id.to_string();
        self.pools
            .run_background(move || {
                if !repo.commit_graph_is_stale() {
                    return false;
                }
                match repo.cli().write_commit_graph() {
                    Ok(()) => true,
                    Err(e) => {
                        tracing::debug!(repo = %id, error = %e, "commit-graph write failed");
                        false
                    }
                }
            })
            .await
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
    async fn refresh_waits_for_a_running_write() {
        let dir = fixture();
        let engine = Arc::new(Engine::new(PoolConfig::default()));
        let (id, _) = engine.open(dir.path()).await.unwrap();
        let before = engine.refresh(&id).await.unwrap().generation;

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let path = dir.path().to_path_buf();
        let writer = {
            let engine = engine.clone();
            let id = id.clone();
            tokio::spawn(async move {
                engine
                    .write(&id, move |_| {
                        std::fs::write(path.join("half"), "x").unwrap();
                        started_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        Ok(())
                    })
                    .await
            })
        };
        tokio::task::spawn_blocking(move || started_rx.recv().unwrap()).await.unwrap();

        let during = engine.refresh(&id).await.unwrap();
        assert_eq!(during.generation, before);
        assert!(during.status.files.is_empty());

        release_tx.send(()).unwrap();
        writer.await.unwrap().unwrap();
        let after = engine.get(&id).unwrap().snapshot();
        assert!(after.generation > before);
        assert!(after.status.files.iter().any(|f| f.path == "half"));
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
    async fn commit_graph_is_written_only_when_stale() {
        let dir = fixture();
        let engine = Engine::new(PoolConfig::default());
        let (id, handle) = engine.open(dir.path()).await.unwrap();
        let graph = handle.repo().common_dir().join("objects/info/commit-graph");
        assert!(!graph.exists());

        assert!(engine.maintain_commit_graph(&id).await.unwrap());
        assert!(graph.is_file());
        assert!(!engine.maintain_commit_graph(&id).await.unwrap());

        GitCli::new(dir.path()).create_branch("topic", None, false).unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        let topic = handle.repo().common_dir().join("refs/heads/topic");
        std::fs::File::options().write(true).open(topic).unwrap().set_modified(later).unwrap();
        assert!(engine.maintain_commit_graph(&id).await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_runs_beside_a_held_write_lock() {
        let dir = fixture();
        let engine = Arc::new(Engine::new(PoolConfig { foreground_threads: 4, ..PoolConfig::default() }));
        let (id, _) = engine.open(dir.path()).await.unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let writer = {
            let engine = engine.clone();
            let id = id.clone();
            tokio::spawn(async move {
                engine
                    .write(&id, move |_| {
                        let _ = started_tx.send(());
                        let _ = release_rx.recv();
                        Ok(())
                    })
                    .await
            })
        };
        started_rx.await.unwrap();
        let fetched = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            engine.fetch(&id, |_| Ok(std::thread::current().name().map(str::to_string))),
        )
        .await
        .expect("fetch waited on the write lock")
        .unwrap();
        assert!(fetched.unwrap_or_default().starts_with("blink-net-"));
        release_tx.send(()).unwrap();
        writer.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn detached_handles_are_not_registered() {
        let dir = fixture();
        let engine = Engine::new(PoolConfig::default());
        let handle = engine.open_detached(dir.path()).await.unwrap();
        let has_remote = engine.fetch_detached(&handle, |repo| Ok(repo.has_remotes())).await.unwrap();
        assert!(!has_remote);
        assert!(engine.open_repos().is_empty());
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
