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

    pub fn read_cleaned_worktree(&self, path: &str) -> Result<Option<Vec<u8>>> {
        use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
        use std::io::Read;

        let file = match std::fs::File::open(self.workdir().join(path)) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let repo = self.local();
        let (mut pipeline, index) = repo.filter_pipeline(None).git()?;
        let mut data = Vec::new();
        match pipeline.convert_to_git(file, std::path::Path::new(path), &index).git()? {
            ToGitOutcome::Unchanged(mut file) => {
                file.read_to_end(&mut data)?;
            }
            ToGitOutcome::Process(mut stream) => {
                stream.read_to_end(&mut data)?;
            }
            ToGitOutcome::Buffer(buf) => data.extend_from_slice(buf),
        }
        Ok(Some(data))
    }

    pub fn gitlink_at(&self, rev: &str, path: &str) -> Result<Option<String>> {
        let repo = self.local();
        let Ok(id) = repo.rev_parse_single(rev) else { return Ok(None) };
        let tree = id.object().git()?.peel_to_tree().git()?;
        let Some(entry) = tree.lookup_entry_by_path(path).git()? else { return Ok(None) };
        Ok(entry.mode().is_commit().then(|| entry.object_id().to_string()))
    }

    fn blob_at_revision(&self, repo: &gix::Repository, rev: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let Ok(id) = repo.rev_parse_single(format!("{rev}:{path}").as_str()) else { return Ok(None) };
        let Ok(object) = id.object() else {
            // Partial clones leave blobs out; the CLI fetches them from the promisor remote on demand.
            return Ok(Some(self.cli().run_bytes(&["cat-file", "blob", &id.to_string()])?));
        };
        if object.kind != gix::object::Kind::Blob {
            return Ok(None);
        }
        Ok(Some(object.detach().data))
    }
}
