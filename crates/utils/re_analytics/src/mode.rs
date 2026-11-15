use crate::{AnalyticsSwitch, user_switch};

/// How an application that embeds the viewer wants analytics to behave.
///
/// The user's [`AnalyticsSwitch`] still wins over [`Self::On`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnalyticsMode {
    /// Analytics on; on the web, the viewer shows a one-time notice about it.
    #[default]
    On,

    /// No analytics.
    Off,
}

impl AnalyticsMode {
    /// The value accepted by [`std::str::FromStr`], as used by the web viewer's `analytics` option.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }

    /// The mode for a web viewer served by this process, e.g. by `rerun --web-viewer`.
    ///
    /// It follows this machine's analytics config.
    pub fn for_web_viewer_served_by_this_process() -> Self {
        match user_switch() {
            AnalyticsSwitch::Enabled => Self::On,
            AnalyticsSwitch::Disabled => Self::Off,
        }
    }

    /// Whether analytics may be sent at all in this mode.
    pub fn allows_analytics(self) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
        }
    }
}

/// The value given for an [`AnalyticsMode`] is neither `on` nor `off`.
#[derive(Debug, thiserror::Error)]
#[error("unknown analytics mode {0:?}, expected \"on\" or \"off\"")]
pub struct UnknownAnalyticsMode(String);

impl std::str::FromStr for AnalyticsMode {
    type Err = UnknownAnalyticsMode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            _ => Err(UnknownAnalyticsMode(s.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_parses_from_its_name() {
        for mode in [AnalyticsMode::On, AnalyticsMode::Off] {
            assert_eq!(mode.as_str().parse::<AnalyticsMode>().ok(), Some(mode));
        }
        assert!(
            "Off".parse::<AnalyticsMode>().is_err(),
            "names are case-sensitive"
        );
    }
}
