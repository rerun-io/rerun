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

/// How to display a [`crate::Timestamp`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimestampFormat {
    /// What kind of format to use.
    format_kind: TimestampFormatKind,

    /// For date-time format kinds, should we omit the date part when it's today?
    ///
    /// By default, we do, but having this toggle is convenient for the uses-cases where omitting
    /// the date part is not desirable.
    hide_today_date: bool,

    /// For date-time format kinds, should we omit date, nanos and suffix?
    short: bool,
}

impl Default for TimestampFormat {
    fn default() -> Self {
        Self {
            format_kind: Default::default(),
            hide_today_date: true,
            short: false,
        }
    }
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

    pub fn with_hide_today_date(mut self, hide_date_when_today: bool) -> Self {
        self.hide_today_date = hide_date_when_today;
        self
    }

    pub fn with_short(mut self, short: bool) -> Self {
        self.short = short;
        self
    }

    pub fn hide_today_date(&self) -> bool {
        self.hide_today_date
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
