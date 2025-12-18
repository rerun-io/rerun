use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use uuid::Uuid;

use crate::Property;

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

    /// Opt-in meta-data you can set via `rerun analytics`.
    ///
    /// For instance Rerun employees are encouraged to set `rerun analytics email`.
    /// For real users, this is always empty.
    #[serde(rename = "metadata", default)]
    pub opt_in_metadata: HashMap<String, Property>,

    /// The path of the config file.
    #[serde(rename = "config_file_path")]
    pub config_file_path: PathBuf,

    /// The directory where pending data is stored.
    #[serde(rename = "data_dir_path")]
    pub data_dir_path: PathBuf,

    /// Is this the first time the user runs the app?
    ///
    /// This is determined based on whether the analytics config already exists on disk.
    #[serde(skip)]
    is_first_run: bool,
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        let dirs = Self::project_dirs()?;
        let config_path = dirs.config_dir().join("analytics.json");
        let data_path = dirs.data_local_dir().join("analytics");
        Ok(Self {
            analytics_id: Uuid::new_v4().simple().to_string(),
            analytics_enabled: true,
            opt_in_metadata: Default::default(),
            session_id: Uuid::new_v4(),
            is_first_run: true,
            config_file_path: config_path,
            data_dir_path: data_path,
        })
    }

    pub fn load_or_default() -> Result<Self, ConfigError> {
        match Self::load()? {
            Some(config) => Ok(config),
            None => Self::new(),
        }
    }

    pub fn load() -> Result<Option<Self>, ConfigError> {
        let dirs = Self::project_dirs()?;
        let config_path = dirs.config_dir().join("analytics.json");
        match File::open(config_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let config = serde_json::from_reader(reader)?;
                Ok(Some(config))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(ConfigError::Io(err)),
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        // create data directory
        std::fs::create_dir_all(self.data_dir())?;

        // create config file
        std::fs::create_dir_all(self.config_dir())?;
        let file = File::create(self.config_file())?;
        serde_json::to_writer(file, self).map_err(Into::into)
    }

    pub fn config_dir(&self) -> &Path {
        self.config_file_path
            .parent()
            .expect("config file has no parent")
    }

    pub fn config_file(&self) -> &Path {
        &self.config_file_path
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir_path
    }

    pub fn is_first_run(&self) -> bool {
        self.is_first_run
    }

    fn project_dirs() -> Result<ProjectDirs, ConfigError> {
        ProjectDirs::from("", "", "rerun").ok_or(ConfigError::UnknownLocation)
    }
}
