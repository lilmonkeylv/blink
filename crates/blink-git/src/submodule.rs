use std::collections::{BTreeMap, HashSet};

use serde::Serialize;

use crate::cli::{GitCli, Progress};
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Submodule {
    pub name: String,
    pub path: String,
    pub url: Option<String>,
    pub initialized: bool,
}

#[derive(Default)]
struct Declared {
    path: Option<String>,
    url: Option<String>,
}

pub fn parse_gitmodules(list: &str) -> Vec<(String, String, Option<String>)> {
    let mut declared: BTreeMap<String, Declared> = BTreeMap::new();
    for entry in list.split('\0') {
        let Some((key, value)) = entry.split_once('\n') else { continue };
        let Some((name, field)) = key.strip_prefix("submodule.").and_then(|k| k.rsplit_once('.')) else { continue };
        let slot = declared.entry(name.to_string()).or_default();
        match field {
            "path" => slot.path = Some(value.to_string()),
            "url" => slot.url = Some(value.to_string()),
            _ => {}
        }
    }
    declared.into_iter().filter_map(|(name, d)| Some((name, d.path?, d.url))).collect()
}

fn gitlinks(ls_files: &str) -> HashSet<&str> {
    ls_files
        .split('\0')
        .filter_map(|entry| entry.split_once('\t'))
        .filter(|(meta, _)| meta.starts_with("160000 "))
        .map(|(_, path)| path)
        .collect()
}

impl GitCli {
    pub fn submodules(&self) -> Result<Vec<Submodule>> {
        if !self.workdir().join(".gitmodules").is_file() {
            return Ok(Vec::new());
        }
        let declared = parse_gitmodules(&self.run(&["config", "-f", ".gitmodules", "--null", "--list"])?);
        if declared.is_empty() {
            return Ok(Vec::new());
        }
        let mut args = vec!["ls-files", "--stage", "-z", "--"];
        args.extend(declared.iter().map(|(_, path, _)| path.as_str()));
        let staged = self.literal_pathspecs().run(&args)?;
        let linked = gitlinks(&staged);
        Ok(declared
            .iter()
            .filter(|(_, path, _)| linked.contains(path.as_str()))
            .map(|(name, path, url)| Submodule {
                name: name.clone(),
                path: path.clone(),
                url: url.clone(),
                initialized: self.workdir().join(path).join(".git").exists(),
            })
            .collect())
    }

    pub fn submodule_init(&self) -> Result<()> {
        self.run(&["submodule", "init"]).map(drop)
    }

    pub fn submodule_sync(&self) -> Result<()> {
        self.run(&["submodule", "sync", "--recursive"]).map(drop)
    }

    pub fn submodule_update(&self, paths: &[&str], progress: impl FnMut(Progress)) -> Result<()> {
        let mut args = vec!["submodule", "update", "--init", "--recursive", "--progress", "--"];
        args.extend_from_slice(paths);
        self.literal_pathspecs().run_with_progress(&args, progress).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_declared_submodules() {
        let list = "submodule.lib.path\nvendor/lib\0submodule.lib.url\nhttps://example.com/lib.git\0submodule.my.dotted.name.path\ndotted\0submodule.orphan.url\nx\0core.bare\nfalse\0";
        assert_eq!(
            parse_gitmodules(list),
            [
                ("lib".into(), "vendor/lib".into(), Some("https://example.com/lib.git".into())),
                ("my.dotted.name".into(), "dotted".into(), None),
            ]
        );
    }

    #[test]
    fn keeps_only_gitlinks() {
        let out = "160000 0123456789abcdef0123456789abcdef01234567 0\tvendor/lib\x001006440123456789abcdef0123456789abcdef01234567 0\tnot-a-module\0";
        let linked = gitlinks(out);
        assert!(linked.contains("vendor/lib"));
        assert!(!linked.contains("not-a-module"));
    }
}
