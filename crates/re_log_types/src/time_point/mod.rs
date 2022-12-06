use std::collections::BTreeMap;

mod arrow;
mod time_int;
mod timeline;

use crate::time::Time;
pub use arrow::*;
pub use time_int::*;
pub use timeline::*;

// ----------------------------------------------------------------------------

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
///
/// If this is empty, the data is _timeless_.
/// Timeless data will show up on all timelines, past and future,
/// and will hit all time queries. In other words, it is always there.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(pub BTreeMap<Timeline, TimeInt>);

impl TimePoint {
    /// Logging to this time means the data will show upp in all timelines,
    /// past and future. The time will be [`TimeInt::BEGINNING`], meaning it will
    /// always be in range for any time query.
    pub fn timeless() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_timeless(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn timelines(&self) -> impl ExactSizeIterator<Item = &Timeline> {
        self.0.keys()
    }
}

// ----------------------------------------------------------------------------

/// The type of a [`TimeInt`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, num_derive::FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeType {
    /// Normal wall time.
    Time,

    /// Used e.g. for frames in a film.
    Sequence,
}

impl TimeType {
    fn hash(&self) -> u64 {
        match self {
            Self::Time => 0,
            Self::Sequence => 1,
        }
    }

    pub fn format(&self, time_int: TimeInt) -> String {
        if time_int <= TimeInt::BEGINNING {
            "-∞".into()
        } else {
            match self {
                Self::Time => Time::from(time_int).format(),
                Self::Sequence => format!("#{}", time_int.0),
            }
        }
    }
}

// ----------------------------------------------------------------------------

// ----------------------------------------------------------------------------

#[inline]
pub fn time_point(
    fields: impl IntoIterator<Item = (&'static str, TimeType, TimeInt)>,
) -> TimePoint {
    TimePoint(
        fields
            .into_iter()
            .map(|(name, tt, ti)| (Timeline::new(name, tt), ti))
            .collect(),
    )
}
