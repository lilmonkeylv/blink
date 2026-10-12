use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::error::Result;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS repositories (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    owner TEXT,
    remote_url TEXT,
    last_opened INTEGER NOT NULL DEFAULT 0,
    added_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS repo_state (
    path TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (path, key)
);
CREATE TABLE IF NOT EXISTS app_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRecord {
    pub path: String,
    pub name: String,
    pub owner: Option<String>,
    pub remote_url: Option<String>,
    pub last_opened: i64,
}

pub struct Cache {
    writer: Mutex<Connection>,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Cache {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { writer: Mutex::new(conn) })
    }

    pub fn add_repository(&self, record: &RepoRecord) -> Result<()> {
        self.writer.lock().execute(
            "INSERT INTO repositories (path, name, owner, remote_url, last_opened, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(path) DO UPDATE SET name = excluded.name, owner = excluded.owner, remote_url = excluded.remote_url",
            params![record.path, record.name, record.owner, record.remote_url, record.last_opened, now()],
        )?;
        Ok(())
    }

    pub fn remove_repository(&self, path: &str) -> Result<()> {
        let conn = self.writer.lock();
        conn.execute("DELETE FROM repositories WHERE path = ?1", [path])?;
        conn.execute("DELETE FROM repo_state WHERE path = ?1", [path])?;
        Ok(())
    }

    pub fn touch_repository(&self, path: &str) -> Result<()> {
        self.writer.lock().execute("UPDATE repositories SET last_opened = ?2 WHERE path = ?1", params![path, now()])?;
        Ok(())
    }

    pub fn repositories(&self) -> Result<Vec<RepoRecord>> {
        let conn = self.writer.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT path, name, owner, remote_url, last_opened FROM repositories ORDER BY owner IS NULL, owner, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(RepoRecord {
                path: r.get(0)?,
                name: r.get(1)?,
                owner: r.get(2)?,
                remote_url: r.get(3)?,
                last_opened: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn put_repo_state<T: Serialize>(&self, path: &str, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.writer.lock().execute(
            "INSERT INTO repo_state (path, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![path, key, json, now()],
        )?;
        Ok(())
    }

    pub fn repo_state<T: for<'de> Deserialize<'de>>(&self, path: &str, key: &str) -> Result<Option<T>> {
        let json: Option<String> = self
            .writer
            .lock()
            .query_row("SELECT value FROM repo_state WHERE path = ?1 AND key = ?2", [path, key], |r| r.get(0))
            .optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }

    pub fn put_app_state<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.writer.lock().execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, json],
        )?;
        Ok(())
    }

    pub fn app_state<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        let json: Option<String> = self
            .writer
            .lock()
            .query_row("SELECT value FROM app_state WHERE key = ?1", [key], |r| r.get(0))
            .optional()?;
        Ok(json.map(|j| serde_json::from_str(&j)).transpose()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, owner: Option<&str>) -> RepoRecord {
        RepoRecord {
            path: path.into(),
            name: path.rsplit('/').next().unwrap().into(),
            owner: owner.map(Into::into),
            remote_url: None,
            last_opened: 0,
        }
    }

    #[test]
    fn repositories_crud() {
        let cache = Cache::in_memory().unwrap();
        cache.add_repository(&record("/x/zeta", Some("acme"))).unwrap();
        cache.add_repository(&record("/x/alpha", None)).unwrap();
        cache.add_repository(&record("/x/beta", Some("acme"))).unwrap();
        let names: Vec<_> = cache.repositories().unwrap().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["beta", "zeta", "alpha"]);
        cache.remove_repository("/x/beta").unwrap();
        assert_eq!(cache.repositories().unwrap().len(), 2);
    }

    #[test]
    fn repo_state_roundtrip() {
        let cache = Cache::in_memory().unwrap();
        cache.put_repo_state("/r", "tab", &"history").unwrap();
        assert_eq!(cache.repo_state::<String>("/r", "tab").unwrap().as_deref(), Some("history"));
        assert_eq!(cache.repo_state::<String>("/r", "none").unwrap(), None);
    }

    #[test]
    fn file_database_uses_wal() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(&dir.path().join("c.db")).unwrap();
        let mode: String = cache.writer.lock().query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        assert_eq!(mode, "wal");
    }
}
