mod cache;
mod desktop;
mod error;
mod paths;
mod secrets;
mod settings;

pub use cache::{Cache, RepoRecord};
pub use desktop::{desktop_indexeddb_dir, desktop_local_storage_dir, desktop_preferences, desktop_repositories};
pub use error::{Error, Result};
pub use paths::AppPaths;
pub use secrets::{KeyringStore, MemoryStore, SecretStore};
pub use settings::*;
