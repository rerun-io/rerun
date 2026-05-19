/// How to display a [`crate::Timestamp`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimestampFormatKind {
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
    SecondsSinceUnixEpoch,
}

/// Controls whether the date part of a timestamp is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DateVisibility {
    /// Always show the date.
    ShowDate,

    /// Hide the date when it's today.
    #[default]
    HideDateToday,

    /// Always hide the date.
    HideDate,
}

/// How to display a [`crate::Timestamp`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimestampFormat {
    /// What kind of format to use.
    format_kind: TimestampFormatKind,

    /// For date-time format kinds, controls whether the date part is shown.
    date_visibility: DateVisibility,

    /// For date-time format kinds, should we omit date, nanos and suffix?
    short: bool,
}

impl From<TimestampFormatKind> for TimestampFormat {
    fn from(value: TimestampFormatKind) -> Self {
        Self {
            format_kind: value,
            ..Default::default()
        }
    }
}

impl TimestampFormat {
    pub fn utc() -> Self {
        Self::from(TimestampFormatKind::Utc)
    }

    pub fn local_timezone() -> Self {
        Self::from(TimestampFormatKind::LocalTimezone)
    }

    pub fn local_timezone_implicit() -> Self {
        Self::from(TimestampFormatKind::LocalTimezoneImplicit)
    }

    pub fn unix_epoch() -> Self {
        Self::from(TimestampFormatKind::SecondsSinceUnixEpoch)
    }

    pub fn kind(&self) -> TimestampFormatKind {
        self.format_kind
    }

    pub fn with_date_visibility(mut self, date_visibility: DateVisibility) -> Self {
        self.date_visibility = date_visibility;
        self
    }

    pub fn with_short(mut self, short: bool) -> Self {
        self.short = short;
        self
    }

    pub fn date_visibility(&self) -> DateVisibility {
        self.date_visibility
    }

    pub fn short(&self) -> bool {
        self.short
    }

    pub fn to_jiff_time_zone(self) -> jiff::tz::TimeZone {
        use jiff::tz::TimeZone;

        match self.format_kind {
            TimestampFormatKind::SecondsSinceUnixEpoch | TimestampFormatKind::Utc => TimeZone::UTC,

            TimestampFormatKind::LocalTimezone | TimestampFormatKind::LocalTimezoneImplicit => {
                TimeZone::try_system().unwrap_or_else(|err| {
                    re_log::warn_once!("Failed to detect system/local time zone: {err}");
                    TimeZone::UTC
                })
            }
        }
    }
}
