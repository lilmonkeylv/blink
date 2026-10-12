use gix::bstr::ByteSlice;

use crate::error::{Result, ResultExt};
use crate::repo::Repository;

#[derive(Debug, Clone)]
pub enum BlobSource {
    Head,
    Index,
    Worktree,
    Revision(String),
}

impl Repository {
    pub fn read_blob(&self, source: &BlobSource, path: &str) -> Result<Option<Vec<u8>>> {
        let repo = self.local();
        match source {
            BlobSource::Worktree => match std::fs::read(self.workdir().join(path)) {
                Ok(data) => Ok(Some(data)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            },
            BlobSource::Index => {
                let index = repo.index_or_empty().git()?;
                let Some(entry) = index.entry_by_path(path.as_bytes().as_bstr()) else { return Ok(None) };
                Ok(Some(repo.find_blob(entry.id).git()?.take_data()))
            }
            BlobSource::Head => self.blob_at_revision(&repo, "HEAD", path),
            BlobSource::Revision(rev) => self.blob_at_revision(&repo, rev, path),
        }
    }

    fn blob_at_revision(&self, repo: &gix::Repository, rev: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let Ok(id) = repo.rev_parse_single(format!("{rev}:{path}").as_str()) else { return Ok(None) };
        let object = id.object().git()?;
        if object.kind != gix::object::Kind::Blob {
            return Ok(None);
        }
        Ok(Some(object.detach().data))
    }
}
