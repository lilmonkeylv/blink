use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use crate::cli::GitCli;
use crate::error::{Error, Result};

const OBJECT_CACHE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct Repository {
    shared: gix::ThreadSafeRepository,
    workdir: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
    remotes: Arc<Mutex<RemoteCache>>,
}

#[derive(Default)]
struct RemoteCache {
    config_modified: Option<SystemTime>,
    remotes: Option<Vec<(String, String)>>,
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository").field("workdir", &self.workdir).finish()
    }
}

impl Repository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let shared =
            gix::ThreadSafeRepository::discover(path).map_err(|_| Error::NotARepository(path.display().to_string()))?;
        let local = shared.to_thread_local();
        let workdir = local.workdir().ok_or(Error::Bare)?.to_path_buf();
        let git_dir = local.git_dir().to_path_buf();
        let common_dir = local.common_dir().to_path_buf();
        Ok(Self {
            shared,
            workdir: dunce(workdir),
            git_dir: dunce(git_dir),
            common_dir: dunce(common_dir),
            remotes: Arc::default(),
        })
    }

    pub fn is_repository(path: impl AsRef<Path>) -> bool {
        gix::ThreadSafeRepository::discover(path.as_ref()).is_ok()
    }

    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    pub fn name(&self) -> String {
        self.workdir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    pub fn cli(&self) -> GitCli {
        GitCli::new(&self.workdir)
    }

    pub(crate) fn local(&self) -> gix::Repository {
        let mut repo = self.shared.to_thread_local();
        repo.object_cache_size_if_unset(OBJECT_CACHE_BYTES);
        repo
    }

    pub fn has_remotes(&self) -> bool {
        !self.remotes().is_empty()
    }

    pub fn remotes(&self) -> Vec<(String, String)> {
        let modified = std::fs::metadata(self.common_dir.join("config")).and_then(|m| m.modified()).ok();
        let mut cache = self.remotes.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.remotes.is_none() || cache.config_modified != modified {
            *cache = RemoteCache { config_modified: modified, remotes: Some(self.read_remotes()) };
        }
        cache.remotes.clone().unwrap_or_default()
    }

    pub fn remote_url(&self, name: &str) -> Option<String> {
        self.remotes().into_iter().find(|(n, _)| n == name).map(|(_, url)| url)
    }

    fn read_remotes(&self) -> Vec<(String, String)> {
        // The shared handle keeps the config it was opened with, so remotes added later need a fresh open.
        let Ok(repo) = gix::open(&self.workdir) else { return Vec::new() };
        repo.remote_names()
            .iter()
            .filter_map(|name| {
                let name: &gix::bstr::BStr = name.as_ref();
                let remote = repo.find_remote(name).ok()?;
                let url = remote.url(gix::remote::Direction::Fetch)?;
                Some((name.to_string(), url.to_bstring().to_string()))
            })
            .collect()
    }

    pub fn is_operation_in_progress(&self) -> Option<&'static str> {
        [
            ("MERGE_HEAD", "merge"),
            ("rebase-merge", "rebase"),
            ("rebase-apply", "rebase"),
            ("CHERRY_PICK_HEAD", "cherry-pick"),
            ("REVERT_HEAD", "revert"),
        ]
        .into_iter()
        .find(|(file, _)| self.git_dir.join(file).exists())
        .map(|(_, op)| op)
    }
}

fn dunce(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\")
            && !rest.starts_with("UNC\\")
        {
            return PathBuf::from(rest);
        }
    }
    path
}
