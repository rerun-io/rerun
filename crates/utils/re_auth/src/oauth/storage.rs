//! Load/store abstraction for credentials on native and Wasm.

use super::Credentials;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("failed to read credentials: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to deserialize credentials: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("could not find a valid config location, please ensure $HOME is set")]
    UnknownConfigLocation,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("failed to write credentials: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to serialize credentials: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("could not find a valid config location, please ensure $HOME is set")]
    UnknownConfigLocation,
}

#[cfg(not(target_arch = "wasm32"))]
pub use file::{load, store};

#[cfg(target_arch = "wasm32")]
pub use web::{load, store};

#[cfg(not(target_arch = "wasm32"))]
mod file {
    use super::{Credentials, LoadError, StoreError};
    use std::path::PathBuf;

    fn credentials_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "rerun")
            .map(|dirs| dirs.config_dir().join("credentials.json"))
    }

    pub fn load() -> Result<Option<Credentials>, LoadError> {
        let path = credentials_path().ok_or(LoadError::UnknownConfigLocation)?;
        let data = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(err) => return Err(err.into()),
        };
        let credentials = serde_json::from_str(&data)?;
        Ok(credentials)
    }

    pub fn store(credentials: &Credentials) -> Result<(), StoreError> {
        let path = credentials_path().ok_or(StoreError::UnknownConfigLocation)?;
        let data = serde_json::to_string_pretty(&credentials)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{Credentials, LoadError, StoreError};

    // const STORAGE_KEY: &'static str = "rerun_auth";

    #[expect(clippy::unnecessary_wraps)] // for compat with non-Wasm
    // TODO(jan): local storage
    pub fn load() -> Result<Option<Credentials>, LoadError> {
        Ok(None)
    }

    pub fn store(credentials: &Credentials) -> Result<(), StoreError> {
        let _ = credentials;
        // This shouldn't actually be called anywhere, because no tokens are stored
        // in local storage, which means nothing to refresh yet, either.
        unreachable!("storing credentials in localStorage is not yet supported")
    }
}
