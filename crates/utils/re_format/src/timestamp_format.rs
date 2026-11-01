use std::ops::RangeInclusive;

/// How to display a timestamp.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize,
)]
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
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum DateVisibility {
    /// Always show the date.
    ShowDate,

    /// Hide the date when it's today.
    #[default]
    HideDateToday,

    /// Always hide the date.
    HideDate,
}

/// How to display a timestamp.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize,
)]
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
    /// How many sub-second decimals [`Self::format`] and [`Self::format_nanos`] show.
    ///
    /// Trailing zeroes are omitted, and sub-microsecond precision is ignored.
    pub const DEFAULT_SUBSECOND_DECIMALS: RangeInclusive<usize> = 0..=6;

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

    /// Human-readable timestamp, with [`Self::DEFAULT_SUBSECOND_DECIMALS`].
    ///
    /// Omits the date of same-day timestamps, unless [`Self::date_visibility`] says otherwise.
    pub fn format(self, timestamp: jiff::Timestamp) -> String {
        self.format_opt(timestamp, Self::DEFAULT_SUBSECOND_DECIMALS)
    }

    /// Like [`Self::format`], for nanoseconds since the Unix epoch.
    pub fn format_nanos(self, nanos_since_epoch: i64) -> String {
        self.format_nanos_opt(nanos_since_epoch, Self::DEFAULT_SUBSECOND_DECIMALS)
    }

    /// Like [`Self::format_opt`], for nanoseconds since the Unix epoch.
    pub fn format_nanos_opt(
        self,
        nanos_since_epoch: i64,
        subsecond_decimals: RangeInclusive<usize>,
    ) -> String {
        match jiff::Timestamp::from_nanosecond(nanos_since_epoch.into()) {
            Ok(timestamp) => self.format_opt(timestamp, subsecond_decimals),
            // jiff covers years -9999 to 9999, which includes every `i64` nanosecond count.
            Err(err) => format!("{nanos_since_epoch}ns ({err})"),
        }
    }

    /// Human-readable timestamp.
    ///
    /// Omits the date of same-day timestamps, unless [`Self::date_visibility`] says otherwise.
    ///
    /// The format will omit trailing sub-second zeroes as far as `subsecond_decimals` permits it.
    pub fn format_opt(
        self,
        timestamp: jiff::Timestamp,
        subsecond_decimals: RangeInclusive<usize>,
    ) -> String {
        let format_fractional_nanos = move |ns: i32| {
            crate::DurationFormatOptions::default()
                .with_always_sign(false)
                .with_min_decimals(*subsecond_decimals.start())
                .with_max_decimals(*subsecond_decimals.end())
                .round_towards_zero()
                .format_nanos(ns as _)
                // Turn `0.123s` into `.123`:
                .trim_start_matches('0')
                .trim_end_matches('s')
                .to_owned()
        };

        match self.kind() {
            TimestampFormatKind::SecondsSinceUnixEpoch => {
                format!(
                    "{}{}",
                    crate::format_int(timestamp.as_second()),
                    format_fractional_nanos(timestamp.subsec_nanosecond())
                )
            }

            TimestampFormatKind::LocalTimezone
            | TimestampFormatKind::LocalTimezoneImplicit
            | TimestampFormatKind::Utc => {
                let tz = self.to_jiff_time_zone();
                let zoned = timestamp.to_zoned(tz.clone());

                let hide_date = match self.date_visibility() {
                    // `ShowDate` takes priority over `short`, so a caller can get a date with
                    // whole seconds, no nanos and no suffix.
                    DateVisibility::ShowDate => false,
                    DateVisibility::HideDate => true,
                    DateVisibility::HideDateToday => {
                        self.short()
                            || zoned.date() == jiff::Timestamp::now().to_zoned(tz.clone()).date()
                    }
                };
                let formatted = if hide_date {
                    zoned.strftime("%H:%M:%S").to_string()
                } else {
                    zoned.strftime("%Y-%m-%d %H:%M:%S").to_string()
                };

                let nanos = if self.short() {
                    String::new()
                } else {
                    format_fractional_nanos(zoned.subsec_nanosecond())
                };

                let suffix = if self.short() {
                    String::new()
                } else {
                    match self.kind() {
                        TimestampFormatKind::LocalTimezone => tz.to_offset(timestamp).to_string(),
                        TimestampFormatKind::LocalTimezoneImplicit => String::new(),
                        TimestampFormatKind::Utc | TimestampFormatKind::SecondsSinceUnixEpoch => {
                            "Z".to_owned()
                        }
                    }
                };

                format!("{formatted}{nanos}{suffix}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_nanos_full_i64_range() {
        let utc = TimestampFormat::utc().with_date_visibility(DateVisibility::ShowDate);
        assert_eq!(utc.format_nanos(0), "1970-01-01 00:00:00Z");
        assert_eq!(utc.format_nanos(-1), "1969-12-31 23:59:59.999\u{2009}999Z");
        assert_eq!(
            utc.format_nanos(i64::MIN),
            "1677-09-21 00:12:43.145\u{2009}224Z"
        );
        assert_eq!(
            utc.format_nanos(i64::MAX),
            "2262-04-11 23:47:16.854\u{2009}775Z"
        );
    }

    #[test]
    fn format_nanos_opt_limits_decimals() {
        let utc = TimestampFormat::utc().with_date_visibility(DateVisibility::ShowDate);
        assert_eq!(
            utc.format_nanos_opt(1_500_000_000, 0..=0),
            "1970-01-01 00:00:01Z"
        );
        assert_eq!(
            utc.format_nanos_opt(1_000_000_000, 3..=3),
            "1970-01-01 00:00:01.000Z"
        );
    }
}
