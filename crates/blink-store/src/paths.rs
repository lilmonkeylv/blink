use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppPaths {
    pub fn system() -> Result<Self> {
        let dirs = directories::ProjectDirs::from("", "", "Blink").ok_or(Error::NoHomeDir)?;
        Ok(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_local_dir().to_path_buf(),
            log_dir: dirs.data_local_dir().join("logs"),
        })
    }

    pub fn rooted(root: &Path) -> Self {
        Self { config_dir: root.join("config"), data_dir: root.join("data"), log_dir: root.join("logs") }
    }

    pub fn ensure(&self) -> Result<()> {
        for dir in [&self.config_dir, &self.data_dir, &self.log_dir] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }

    pub fn cache_db(&self) -> PathBuf {
        self.data_dir.join("cache.sqlite3")
    }
}
