use re_log_types::{NonMinI64, ResolvedTimeRangeF, TimeCell};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TimeRange {
    pub timeline: re_log_types::Timeline,
    pub min: NonMinI64,
    pub max: NonMinI64,
}

impl From<TimeRange> for ResolvedTimeRangeF {
    fn from(range: TimeRange) -> Self {
        Self {
            min: range.min.into(),
            max: range.max.into(),
        }
    }
}

impl std::fmt::Display for TimeRange {
    /// Used for formatting time ranges in URLs
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { timeline, min, max } = self;

        let min = TimeCell::new(timeline.typ(), *min);
        let max = TimeCell::new(timeline.typ(), *max);

        let name = timeline.name();
        write!(f, "{name}@{min}..{max}")
    }
}

impl std::str::FromStr for TimeRange {
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
        let min = min.into();
        let max = max.into();

        Ok(Self { timeline, min, max })
    }
}
