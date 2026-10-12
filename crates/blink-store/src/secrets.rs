use std::collections::HashMap;

use parking_lot::Mutex;

use crate::error::{Error, Result};

pub const SERVICE: &str = "dev.blink.app";

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, secret: &str) -> Result<()>;
    fn delete(&self, account: &str) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct KeyringStore;

fn entry(account: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, account).map_err(|e| Error::Keyring(e.to_string()))
}

impl SecretStore for KeyringStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match entry(account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Keyring(e.to_string())),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        entry(account)?.set_password(secret).map_err(|e| Error::Keyring(e.to_string()))
    }

    fn delete(&self, account: &str) -> Result<()> {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Keyring(e.to_string())),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        Ok(self.0.lock().get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        self.0.lock().insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<()> {
        self.0.lock().remove(account);
        Ok(())
    }
}
