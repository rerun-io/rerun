use crate::{NonMinI64, TimeType};

use super::TimeInt;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
/// An typed cell of an index, e.g. a point in time on some unknown timeline.
pub struct IndexCell {
    pub typ: TimeType,
    pub value: NonMinI64,
}

impl IndexCell {
    pub const ZERO_DURATION: Self = Self {
        typ: TimeType::Time,
        value: NonMinI64::ZERO,
    };

    pub const ZERO_SEQUENCE: Self = Self {
        typ: TimeType::Sequence,
        value: NonMinI64::ZERO,
    };

    #[inline]
    pub fn new(typ: TimeType, value: impl TryInto<NonMinI64>) -> Self {
        let value = value.try_into().unwrap_or(NonMinI64::MIN); // clamp to valid range
        Self { typ, value }
    }

    #[inline]
    pub fn from_sequence(sequence: impl TryInto<NonMinI64>) -> Self {
        Self::new(TimeType::Sequence, sequence)
    }

    #[inline]
    pub fn from_duration_nanos(nanos: impl TryInto<NonMinI64>) -> Self {
        Self::new(TimeType::Time, nanos)
    }

    #[inline]
    pub fn from_timestamp_nanos_since_epoch(nanos_since_epoch: impl TryInto<NonMinI64>) -> Self {
        Self::new(TimeType::Time, nanos_since_epoch)
    }

    #[inline]
    pub fn typ(&self) -> TimeType {
        self.typ
    }

    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.value.into()
    }

    #[inline]
    pub fn as_time_int(&self) -> TimeInt {
        TimeInt::from(self.value)
    }
}

impl re_byte_size::SizeBytes for IndexCell {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl TryFrom<std::time::Duration> for IndexCell {
    type Error = std::num::TryFromIntError;

    fn try_from(time: std::time::Duration) -> Result<Self, Self::Error> {
        i64::try_from(time.as_nanos()).map(Self::from_timestamp_nanos_since_epoch)
    }
}

impl TryFrom<std::time::SystemTime> for IndexCell {
    type Error = std::time::SystemTimeError;

    fn try_from(time: std::time::SystemTime) -> Result<Self, Self::Error> {
        let duration_since_epoch = time.duration_since(std::time::SystemTime::UNIX_EPOCH)?;
        let nanos_since_epoch = duration_since_epoch.as_nanos();
        let nanos_since_epoch = nanos_since_epoch as i64; // TODO: saturating cast
        Ok(Self::from_timestamp_nanos_since_epoch(nanos_since_epoch))
    }
}

// On non-wasm32 builds, `web_time::SystemTime` is a re-export of `std::time::SystemTime`,
// so it's covered by the above `TryFrom`.
#[cfg(target_arch = "wasm32")]
impl TryFrom<web_time::SystemTime> for IndexCell {
    type Error = web_time::SystemTimeError;

    fn try_from(time: web_time::SystemTime) -> Result<Self, Self::Error> {
        time.duration_since(web_time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Self(duration_since_epoch.as_nanos() as _))
    }
}
