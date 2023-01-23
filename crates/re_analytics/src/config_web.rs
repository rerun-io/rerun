use uuid::Uuid;

// ---

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
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

    /// Is this the first time the user runs the app?
    ///
    /// This is determined based on whether the analytics config already exists on disk.
    #[serde(skip)]
    is_first_run: bool,
}

impl Config {
    pub fn load() -> Result<Config, ConfigError> {
        if let Some(config_str) = local_storage_get(Self::config_key()) {
            serde_json::from_str(&config_str).map_err(Into::into)
        } else {
            Ok(Config {
                analytics_id: Uuid::new_v4().to_string(),
                analytics_enabled: true,
                session_id: Uuid::new_v4(),
                is_first_run: true,
            })
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let config_str = serde_json::to_string(self)?;
        local_storage_set(Self::config_key(), &config_str);
        Ok(())
    }

    pub fn config_key() -> &'static str {
        "rerun_analytics_config"
    }
    pub fn data_key() -> &'static str {
        "rerun_analytics_data"
    }

    pub fn is_first_run(&self) -> bool {
        self.is_first_run
    }
}

// ---

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn local_storage_get(key: &str) -> Option<String> {
    local_storage().map(|storage| storage.get_item(key).ok())??
}

pub fn local_storage_set(key: &str, value: &str) {
    local_storage().map(|storage| storage.set_item(key, value));
}
