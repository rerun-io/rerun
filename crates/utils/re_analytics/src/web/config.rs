#![expect(clippy::unused_self)]

use std::collections::HashMap;

use uuid::Uuid;
use web_sys::Storage;

use crate::Property;

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("failed to get localStorage")]
    NoStorage,

    #[error("{0}")]
    Storage(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    // NOTE: not a UUID on purpose, it is sometimes useful to use handcrafted IDs.
    #[serde(rename = "analytics_id")]
    pub analytics_id: String,

    /// A unique ID for this session.
    #[serde(skip, default = "::uuid::Uuid::new_v4")]
    pub session_id: Uuid,

    #[serde(rename = "metadata", default)]
    pub opt_in_metadata: HashMap<String, Property>,
}

fn get_local_storage() -> Result<Storage, ConfigError> {
    let window = web_sys::window().ok_or(ConfigError::NoStorage)?;
    let Ok(Some(storage)) = window.local_storage() else {
        return Err(ConfigError::NoStorage);
    };
    Ok(storage)
}

impl Config {
    const STORAGE_KEY: &'static str = "rerun_config";

    #[expect(clippy::unnecessary_wraps)]
    pub fn new() -> Result<Self, ConfigError> {
        Ok(Self::default())
    }

    #[expect(clippy::map_err_ignore)]
    pub fn load() -> Result<Option<Self>, ConfigError> {
        let storage = get_local_storage()?;
        let value = storage
            .get_item(Self::STORAGE_KEY)
            .map_err(|_| ConfigError::Storage(format!("failed to get {:?}", Self::STORAGE_KEY)))?;
        match value {
            Some(value) => Ok(Some(serde_json::from_str(&value)?)),
            None => Ok(None),
        }
    }

    pub fn load_or_default() -> Result<Self, ConfigError> {
        match Self::load()? {
            Some(config) => Ok(config),
            None => Self::new(),
        }
    }

    #[expect(clippy::map_err_ignore)]
    pub fn save(&self) -> Result<(), ConfigError> {
        let storage = get_local_storage()?;
        let string = serde_json::to_string(self)?;
        storage
            .set_item(Self::STORAGE_KEY, &string)
            .map_err(|_| ConfigError::Storage(format!("failed to set {:?}", Self::STORAGE_KEY)))
    }

    pub fn is_first_run(&self) -> bool {
        // no first-run opt-out for web

        false
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            analytics_id: Uuid::new_v4().simple().to_string(),
            session_id: Uuid::new_v4(),
            opt_in_metadata: HashMap::new(),
        }
    }
}
