//! `relint.toml`: per-project configuration.

use std::path::{Path, PathBuf};

/// Name of the configuration file. A file uses the one in its nearest ancestor directory.
pub const FILE_NAME: &str = "relint.toml";

/// Per-project configuration.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Rules that do not apply to this project.
    #[serde(default)]
    pub disabled_rules: Vec<String>,
}

/// Why a `relint.toml` could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The file could not be read.
    #[error("Failed to read config: {source}\nFile path: {}", path.display())]
    Read {
        /// The config file.
        path: PathBuf,

        /// The I/O error.
        source: std::io::Error,
    },

    /// The file is not a valid config.
    #[error("Failed to parse config: {source}\nFile path: {}", path.display())]
    Parse {
        /// The config file.
        path: PathBuf,

        /// The TOML error.
        source: toml::de::Error,
    },
}

impl Config {
    /// Load the configuration, or the default one if the file does not exist.
    pub fn load(path: &Path) -> Result<Self, LoadError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(|source| LoadError::Read {
            path: path.to_owned(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| LoadError::Parse {
            path: path.to_owned(),
            source,
        })
    }
}
