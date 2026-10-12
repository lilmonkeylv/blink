#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid settings file: {0}")]
    SettingsParse(#[from] toml::de::Error),
    #[error(transparent)]
    SettingsWrite(#[from] toml::ser::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("keychain: {0}")]
    Keyring(String),
    #[error("could not determine user directories")]
    NoHomeDir,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
