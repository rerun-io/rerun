use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Config, ConfigError, analytics_forced};

/// Whether the user allows analytics, as saved in their analytics config.
///
/// Set with `rerun analytics enable|disable` on native, or from the viewer's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyticsSwitch {
    /// Analytics may be sent, also when there is no saved config yet.
    Enabled,

    /// The user opted out.
    Disabled,
}

/// The user's switch, loaded from the config on first use.
static USER_SWITCH_ENABLED: OnceLock<AtomicBool> = OnceLock::new();

fn user_switch_enabled() -> &'static AtomicBool {
    USER_SWITCH_ENABLED
        .get_or_init(|| AtomicBool::new(load_user_switch() == AnalyticsSwitch::Enabled))
}

/// Set the switch from an already loaded config, unless it has been read already.
pub fn init_from_config(config: &Config) {
    USER_SWITCH_ENABLED
        .get_or_init(|| AtomicBool::new(config.analytics_enabled || analytics_forced()));
}

fn load_user_switch() -> AnalyticsSwitch {
    // Tests must never read the shared config.
    if cfg!(any(test, feature = "testing")) {
        return AnalyticsSwitch::Enabled;
    }
    if analytics_forced() {
        return AnalyticsSwitch::Enabled;
    }
    match Config::load() {
        Ok(Some(config)) if !config.analytics_enabled => AnalyticsSwitch::Disabled,
        Ok(_) => AnalyticsSwitch::Enabled,
        Err(err) => {
            re_log::debug!("Failed to load the analytics config: {err}");
            AnalyticsSwitch::Enabled
        }
    }
}

/// The user's choice, as saved in their analytics config.
///
/// A missing config counts as [`AnalyticsSwitch::Enabled`], as it does on first run.
pub fn user_switch() -> AnalyticsSwitch {
    if user_switch_enabled().load(Ordering::Relaxed) {
        AnalyticsSwitch::Enabled
    } else {
        AnalyticsSwitch::Disabled
    }
}

/// Save the user's choice to their analytics config.
///
/// Turning analytics off takes effect immediately.
/// On native, turning it back on takes effect from the next start, because the pipeline is only
/// created at startup.
pub fn set_user_switch(switch: AnalyticsSwitch) -> Result<(), ConfigError> {
    let enabled = switch == AnalyticsSwitch::Enabled;
    if !cfg!(any(test, feature = "testing")) {
        let mut config = Config::load_or_default()?;
        config.analytics_enabled = enabled;
        config.save()?;
    }
    user_switch_enabled().store(enabled, Ordering::Relaxed);
    Ok(())
}
