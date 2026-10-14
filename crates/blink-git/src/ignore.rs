use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;
use gix::worktree::stack::state::ignore::Source;

use crate::repo::Repository;

impl Repository {
    pub fn retain_unignored(&self, paths: &mut BTreeSet<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let repo = self.local();
        let Ok(index) = repo.index_or_empty() else { return };
        let Ok(mut excludes) = repo.excludes(&index, None, Source::WorktreeThenIdMappingIfNotSkipped) else { return };
        paths.retain(|rel| !is_ignored(self.workdir(), &mut excludes, &index, rel));
    }
}

fn is_ignored(workdir: &Path, excludes: &mut gix::AttributeStack<'_>, index: &gix::index::State, rel: &Path) -> bool {
    if rel.file_name().is_some_and(|name| name == ".gitignore") {
        return false;
    }
    let Ok(key) = gix::path::into_bstr(rel) else { return false };
    if index.entry_by_path(gix::path::to_unix_separators_on_windows(key).as_bstr()).is_some() {
        return false;
    }
    let mut prefix = PathBuf::new();
    let mut components = rel.components().peekable();
    while let Some(component) = components.next() {
        prefix.push(component);
        let is_dir = components.peek().is_some() || workdir.join(&prefix).is_dir();
        match excludes.at_path(&prefix, is_dir.then_some(gix::index::entry::Mode::DIR)) {
            Ok(platform) if platform.is_excluded() => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}
