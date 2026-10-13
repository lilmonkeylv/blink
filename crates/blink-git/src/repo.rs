use std::path::{Path, PathBuf};

use crate::cli::GitCli;
use crate::error::{Error, Result};

const OBJECT_CACHE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct Repository {
    shared: gix::ThreadSafeRepository,
    workdir: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
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
        Ok(Self { shared, workdir: dunce(workdir), git_dir: dunce(git_dir), common_dir: dunce(common_dir) })
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
        !self.local().remote_names().is_empty()
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
