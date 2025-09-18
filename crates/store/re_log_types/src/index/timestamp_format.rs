/// How to display a [`crate::Timestamp`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimestampFormat {
    /// Convert to the local timezone and display as such explicitly (e.g. with "+01" for CET).
    LocalTimezone,

    /// Convert to the local timezone and display as such without specifying the timezone.
    ///
    /// Note that in this case the representation is ambiguous.
    LocalTimezoneImplicit,

    /// Display as UTC.
    #[default]
    Utc,

    /// Show as seconds since unix epoch
    UnixEpoch,
}

impl TimestampFormat {
    pub fn to_jiff_time_zone(self) -> jiff::tz::TimeZone {
        use jiff::tz::TimeZone;

        match self {
            Self::UnixEpoch | Self::Utc => TimeZone::UTC,

            Self::LocalTimezone | Self::LocalTimezoneImplicit => TimeZone::try_system()
                .unwrap_or_else(|err| {
                    re_log::warn_once!("Failed to detect system/local time zone: {err}");
                    TimeZone::UTC
                }),
        }
    }
}
