use re_log_types::{AbsoluteTimeRange, AbsoluteTimeRangeF, TimeCell};

use crate::Error;

/// A time range selection as used in URIs, qualified with a timeline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TimeSelection {
    pub timeline: re_log_types::Timeline,
    pub range: AbsoluteTimeRange,
}

impl From<TimeSelection> for AbsoluteTimeRangeF {
    fn from(range: TimeSelection) -> Self {
        range.range.into()
    }
}

impl From<TimeSelection> for AbsoluteTimeRange {
    fn from(range: TimeSelection) -> Self {
        range.range
    }
}

impl std::fmt::Display for TimeSelection {
    /// Used for formatting time ranges in URLs
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { timeline, range } = self;

        let min = TimeCell::new(timeline.typ(), range.min());
        let max = TimeCell::new(timeline.typ(), range.max());

        let name = timeline.name();
        write!(f, "{name}@{min}..{max}")
    }
}

impl std::str::FromStr for TimeSelection {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (timeline, range) = value
            .split_once('@')
            .ok_or_else(|| Error::InvalidTimeRange("Missing @".to_owned()))?;

        let (min, max) = range
            .split_once("..")
            .ok_or_else(|| Error::InvalidTimeRange("Missing ..".to_owned()))?;

        let min = min.parse::<TimeCell>().map_err(|err| {
            Error::InvalidTimeRange(format!("Failed to parse time index '{min}': {err}"))
        })?;
        let max = max.parse::<TimeCell>().map_err(|err| {
            Error::InvalidTimeRange(format!("Failed to parse time index '{max}': {err}"))
        })?;

        if min.typ() != max.typ() {
            return Err(Error::InvalidTimeRange(format!(
                "min/max had differing types. Min was identified as {}, whereas max was identified as {}",
                min.typ(),
                max.typ()
            )));
        }

        let timeline = re_log_types::Timeline::new(timeline, min.typ());
        let range = AbsoluteTimeRange::new(min, max);

        Ok(Self { timeline, range })
    }
}
