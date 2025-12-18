use std::path::Path;

use crate::{AnalyticsError, Config, ConfigError, Property};

// ---

#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Analytics(#[from] AnalyticsError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

pub fn clear() -> Result<(), CliError> {
    let config = Config::load_or_default()?;

    fn delete_dir(dir: &Path) -> Result<(), CliError> {
        eprint!("Are you sure you want to delete directory {dir:?}? [y/N]: ",);

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim() == "y" {
            if let Err(err) = std::fs::remove_dir_all(dir)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                return Err(err.into());
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

pub fn set(props: impl IntoIterator<Item = (String, Property)>) -> Result<(), CliError> {
    let mut config = Config::load_or_default()?;
    config.opt_in_metadata.extend(props);
    config.save().map_err(Into::into)
}

pub fn opt(enabled: bool) -> Result<(), CliError> {
    let mut config = Config::load_or_default()?;
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
    * Anonymous Usage Data Collection in Rerun *

    Opting out:
    - Run `rerun analytics disable` to opt out of all usage data collection.

    What data is collected?
    - The exact set of analytics events and parameters can be found here:
      https://github.com/rerun-io/rerun/blob/GIT_HASH/crates/utils/re_analytics/src/event.rs
    - We collect high level events about the usage of the Rerun Viewer. For example:
      - The event 'Viewer Opened' helps us estimate how often Rerun is used.
      - The event 'Data Source Connected' helps us understand if users tend to use live
        data sources or recordings most, which helps us prioritize features.
    - We associate events with:
        - Metadata about the Rerun build (version, target platform, etc).
        - A persistent random id that is used to associate events from
          multiple sessions together. To regenerate it run `rerun analytics clear`.
    - We may associate these events with a hashed `application_id` and `recording_id`,
      so that we can understand if users are more likely to look at few applications often,
      or tend to use Rerun for many temporary scripts. Again, this helps us prioritize.
    - We may for instance add events that help us understand how well the auto-layout works.

    What data is not collected?
    - No Personally Identifiable Information, such as user name or IP address, is collected.
      - This assumes you don't manually and explicitly associate your email with
        the analytics events using the analytics helper cli.
        (Don't do this, it's just meant for internal use for the Rerun team.)
    - No user data logged to Rerun is collected.
      - In some cases we collect secure hashes of user provided names (e.g. `application_id`),
        but take great care do this only when we have a clear understanding of why it's needed
        and it won't risk leaking anything potentially proprietary.

    Why do we collect data?
    - To improve the Rerun open source library.

    Usage data we do collect will be sent to and stored in servers within the EU.

    You can audit the actual data being sent out by inspecting the Rerun data directory directly.
    Find out its location by running `rerun analytics config`.
";

pub fn print_details(git_hash_or_tag: &str) {
    eprintln!("{}", DETAILS.replace("GIT_HASH", git_hash_or_tag));
}
