use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use directories_next::ProjectDirs;
use uuid::Uuid;

// ---

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("Couldn't compute config location")]
    UnknownLocation,

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),
}

// pub type WriteResult<T> = ::std::result::Result<T, WriteError>;

// TODO: I guess it better be named UserConfig
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub analytics_enabled: bool,
    // TODO: explain that this is _not_ a UUID on purpose.
    pub analytics_id: String,

    // TODO: explain
    // TODO: probably better served by a time-based uuid there
    #[serde(skip, default = "::uuid::Uuid::new_v4")]
    pub session_id: Uuid,

    pub path: PathBuf,
    // TODO: gotta be in XDG_DATA!!!
    pub data_path: PathBuf,

    // TODO: explain
    // TODO: never written, so default to false when read from disk.
    #[serde(skip)]
    is_first_run: bool,
}

impl Config {
    pub fn load() -> Result<Config, ConfigError> {
        let dirs = Self::project_dirs()?;
        let config_path = dirs.config_dir().join("analytics.json");
        let data_path = dirs.data_local_dir().join("analytics");
        let config = match File::open(&config_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                serde_json::from_reader(reader)?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Config {
                analytics_id: Uuid::new_v4().to_string(),
                analytics_enabled: true,
                session_id: Uuid::new_v4(),
                is_first_run: true,
                path: config_path,
                data_path,
            },
            Err(err) => return Err(ConfigError::IoError(err)),
        };

        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        // create data directory
        std::fs::create_dir_all(&self.data_path)?;

        // create config file
        std::fs::create_dir_all(self.path.parent().unwrap())?;
        let file = File::create(&self.path)?;
        serde_json::to_writer(file, self).map_err(Into::into)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn data_path(&self) -> &Path {
        &self.data_path
    }

    pub fn is_first_run(&self) -> bool {
        self.is_first_run
    }

    fn project_dirs() -> Result<ProjectDirs, ConfigError> {
        ProjectDirs::from("", "", "rerun").ok_or(ConfigError::UnknownLocation)
    }
}
