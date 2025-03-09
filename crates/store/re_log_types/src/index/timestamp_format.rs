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
