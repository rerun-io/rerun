//! Load/store abstraction for credentials on native and Wasm.

use super::Credentials;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("failed to read credentials: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to deserialize credentials: {0}")]
    Serde(#[from] serde_json::Error),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("could not find a valid config location, please ensure $HOME is set")]
    UnknownConfigLocation,

    #[cfg(target_arch = "wasm32")]
    #[error("failed to get window.localStorage")]
    NoLocalStorage,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("failed to write credentials: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to serialize credentials: {0}")]
    Serde(#[from] serde_json::Error),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("could not find a valid config location, please ensure $HOME is set")]
    UnknownConfigLocation,

    #[cfg(target_arch = "wasm32")]
    #[error("failed to get window.localStorage")]
    NoLocalStorage,
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
        Ok(Some(credentials))
    }

    pub fn store(credentials: &Credentials) -> Result<(), StoreError> {
        let path = credentials_path().ok_or(StoreError::UnknownConfigLocation)?;
        let data = serde_json::to_string_pretty(credentials)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::JsCast as _;

    use super::{Credentials, LoadError, StoreError};

    const STORAGE_KEY: &'static str = "rerun_auth";

    struct NoLocalStorage;

    impl From<NoLocalStorage> for LoadError {
        fn from(_: NoLocalStorage) -> Self {
            Self::NoLocalStorage
        }
    }

    impl From<NoLocalStorage> for StoreError {
        fn from(_: NoLocalStorage) -> Self {
            Self::NoLocalStorage
        }
    }

    #[expect(clippy::needless_pass_by_value)]
    pub fn string_from_js_value(s: wasm_bindgen::JsValue) -> String {
        // it's already a string
        if let Some(s) = s.as_string() {
            return s;
        }

        // it's an Error, call `toString` instead
        if let Some(s) = s.dyn_ref::<js_sys::Error>() {
            return format!("{}", s.to_string());
        }

        format!("{s:#?}")
    }

    fn get_local_storage() -> Result<web_sys::Storage, NoLocalStorage> {
        web_sys::window()
            .ok_or(NoLocalStorage)?
            .local_storage()
            .map_err(|_| NoLocalStorage)?
            .ok_or(NoLocalStorage)
    }

    #[expect(clippy::unnecessary_wraps)] // for compat with non-Wasm
    // TODO(jan): local storage
    pub fn load() -> Result<Option<Credentials>, LoadError> {
        let local_storage = get_local_storage()?;
        let data = local_storage.get_item(STORAGE_KEY).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::Other, string_from_js_value(err))
        })?;

        let Some(data) = data else {
            return Ok(None);
        };

        let credentials = serde_json::from_str(&data)?;
        Ok(Some(credentials))
    }

    pub fn store(credentials: &Credentials) -> Result<(), StoreError> {
        let local_storage = get_local_storage()?;
        let data = serde_json::to_string(credentials)?;
        local_storage.set_item(STORAGE_KEY, &data).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::Other, string_from_js_value(err))
        })?;
        Ok(())
    }
}
