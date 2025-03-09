/// How to display a [`Time`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeZone {
    /// Convert to local timezone and display as such.
    Local,

    /// Display as UTC.
    Utc,

    /// Show as seconds since unix epoch
    UnixEpoch,
}
