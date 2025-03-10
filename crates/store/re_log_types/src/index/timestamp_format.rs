/// How to display a [`crate::Timestamp`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimestampFormat {
    /// Convert to local timezone and display as such.
    LocalTimezone,

    /// Display as UTC.
    Utc,

    /// Show as seconds since unix epoch
    UnixEpoch,
}

impl TimestampFormat {
    pub fn to_jiff_time_zone(self) -> jiff::tz::TimeZone {
        use jiff::tz::TimeZone;

        match self {
            Self::UnixEpoch | Self::Utc => TimeZone::UTC,

            Self::LocalTimezone => match TimeZone::try_system() {
                Ok(tz) => tz,
                Err(err) => {
                    re_log::warn_once!("Failed to detect system/local time zone: {err}");
                    TimeZone::UTC
                }
            },
        }
    }
}
