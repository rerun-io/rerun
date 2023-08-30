#![allow(clippy::todo, clippy::unused_self)]

use std::collections::HashMap;

use uuid::Uuid;

use crate::Property;

// ---

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("Couldn't compute config location")]
    UnknownLocation,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

// NOTE: all the `rename` clauses are to avoid a potential catastrophe :)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(rename = "analytics_enabled")]
    pub analytics_enabled: bool,

    // NOTE: not a UUID on purpose, it is sometimes useful to use handcrafted IDs.
    #[serde(rename = "analytics_id")]
    pub analytics_id: String,

    /// A unique ID for this session.
    #[serde(skip, default = "::uuid::Uuid::new_v4")]
    pub session_id: Uuid,

    #[serde(rename = "metadata", default)]
    pub metadata: HashMap<String, Property>,
}

impl Config {
    pub fn load() -> Result<Config, ConfigError> {
        todo!("web support")
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        todo!("web support")
    }

    pub fn is_first_run(&self) -> bool {
        todo!("web support")
    }
}
