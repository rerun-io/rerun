use std::path::Path;

use crate::{Config, ConfigError};
// ---

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

pub fn clear() -> Result<(), CliError> {
    let config = Config::load()?;

    fn delete_dir(dir: &Path) -> Result<(), CliError> {
        eprint!("Are you sure you want to delete directory {dir:?}? [y/N]: ",);

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim() == "y" {
            if let Err(err) = std::fs::remove_dir_all(dir) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    return Err(err.into());
                }
            }
            eprintln!("Deleted {dir:?}");
        }

        Ok(())
    }

    // clear config dir
    delete_dir(config.config_dir())?;

    // clear data dir
    delete_dir(config.data_dir())?;

    Ok(())
}

pub fn opt(enabled: bool) -> Result<(), CliError> {
    let mut config = Config::load()?;
    config.analytics_enabled = enabled;
    config.save()?;

    if enabled {
        eprintln!("Analytics enabled");
    } else {
        eprintln!("Analytics disabled");
    }

    Ok(())
}

pub fn print_config() -> Result<(), CliError> {
    let config = Config::load()?;
    serde_json::to_writer_pretty(std::io::stdout(), &config).map_err(Into::into)
}

const DETAILS: &str = "
    If you'd like to opt out, run the following: `rerun analytics disable`.

    You can check out all of our telemetry events in `re_analytics/src/events.rs`.

    You can audit the actual data being sent out by inspecting the Rerun data directory directly.
    Find out its location by running `rerun analytics config`.
";

pub fn print_details() {
    eprintln!("{DETAILS}");
}
