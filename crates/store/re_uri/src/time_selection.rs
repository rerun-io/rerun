use re_log_types::{AbsoluteTimeRange, AbsoluteTimeRangeF, TimeCell, Timeline};

use crate::Error;

/// A time range selection as used in URIs, qualified with a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TimeSelection {
    pub timeline: Timeline,
    pub range: AbsoluteTimeRange,
}

impl TimeSelection {
    pub fn format(&self, timestamp_format: re_log_types::TimestampFormat) -> String {
        format!(
            "{}-{}",
            TimeCell::new(self.timeline.typ(), self.range.min).format_compact(timestamp_format),
            TimeCell::new(self.timeline.typ(), self.range.max).format_compact(timestamp_format),
        )
    }

    pub fn url_format(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { timeline, range } = self;

        let min = TimeCell::new(timeline.typ(), range.min());
        let max = TimeCell::new(timeline.typ(), range.max());

        let name = timeline.name();
        write!(f, "{name}@")?;

        min.url_format(f)?;
        write!(f, "..")?;
        max.url_format(f)
    }
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

        let timeline = Timeline::new(timeline, min.typ());
        let range = AbsoluteTimeRange::new(min, max);

        Ok(Self { timeline, range })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use re_log_types::TimeInt;

    use super::*;

    #[test]
    fn test_parse_format_time_selection() {
        let test_cases = [
            (
                "sequence@1096..2097",
                TimeSelection {
                    timeline: Timeline::new_sequence("sequence"),
                    range: AbsoluteTimeRange {
                        min: TimeInt::from_sequence(1096.try_into().unwrap()),
                        max: TimeInt::from_sequence(2097.try_into().unwrap()),
                    },
                },
            ),
            (
                "duration@+1.096s..+2.097s",
                TimeSelection {
                    timeline: Timeline::new_duration("duration"),
                    range: AbsoluteTimeRange {
                        min: TimeInt::from_secs(1.096),
                        max: TimeInt::from_secs(2.097),
                    },
                },
            ),
            (
                "duration@-1.096s..+2.097s",
                TimeSelection {
                    timeline: Timeline::new_duration("duration"),
                    range: AbsoluteTimeRange {
                        min: TimeInt::from_secs(-1.096),
                        max: TimeInt::from_secs(2.097),
                    },
                },
            ),
            (
                "duration@−1.096s..+2.097s", // NOTE: special minus character: https://www.compart.com/en/unicode/U+2212
                TimeSelection {
                    timeline: Timeline::new_duration("duration"),
                    range: AbsoluteTimeRange {
                        min: TimeInt::from_secs(-1.096),
                        max: TimeInt::from_secs(2.097),
                    },
                },
            ),
        ];

        struct UrlFormat(TimeSelection);

        impl std::fmt::Display for UrlFormat {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.url_format(f)
            }
        }

        for (string, selection) in test_cases {
            assert_eq!(TimeSelection::from_str(string), Ok(selection));
            assert_eq!(
                TimeSelection::from_str(&UrlFormat(selection).to_string()).unwrap(),
                selection
            );
        }
    }
}
