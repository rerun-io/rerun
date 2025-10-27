use std::ops::RangeInclusive;

use crate::{TimeInt, TimeReal};

// ----------------------------------------------------------------------------

/// An absolute time range using [`TimeInt`].
///
/// Can be resolved from [`re_types_core::datatypes::TimeRange`] (which *may* have relative bounds) using a given timeline & cursor.
///
/// Should not include [`TimeInt::STATIC`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AbsoluteTimeRange {
    pub min: TimeInt,
    pub max: TimeInt,
}

impl AbsoluteTimeRange {
    /// Contains no time at all.
    pub const EMPTY: Self = Self {
        min: TimeInt::MAX,
        max: TimeInt::MIN,
    };

    /// Contains all time.
    pub const EVERYTHING: Self = Self {
        min: TimeInt::MIN,
        max: TimeInt::MAX,
    };

    /// Creates a new temporal [`AbsoluteTimeRange`].
    ///
    /// The returned range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn new(min: impl TryInto<TimeInt>, max: impl TryInto<TimeInt>) -> Self {
        let min = TimeInt::saturated_temporal(min);
        let max = TimeInt::saturated_temporal(max);
        Self { min, max }
    }

    /// The returned range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn point(time: impl TryInto<TimeInt>) -> Self {
        let time = TimeInt::saturated_temporal(time);
        Self {
            min: time,
            max: time,
        }
    }

    #[inline]
    pub fn min(&self) -> TimeInt {
        self.min
    }

    #[inline]
    pub fn max(&self) -> TimeInt {
        self.max
    }

    /// Overwrites the start bound of the range.
    ///
    /// The resulting range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn set_min(&mut self, time: impl TryInto<TimeInt>) {
        let time = TimeInt::saturated_temporal(time);
        self.min = time;
    }

    /// Overwrites the end bound of the range.
    ///
    /// The resulting range is guaranteed to never include [`TimeInt::STATIC`].
    #[inline]
    pub fn set_max(&mut self, time: impl TryInto<TimeInt>) {
        let time = TimeInt::saturated_temporal(time);
        self.max = time;
    }

    /// The amount of time or sequences covered by this range.
    #[inline]
    pub fn abs_length(&self) -> u64 {
        self.min.as_i64().abs_diff(self.max.as_i64())
    }

    #[inline]
    pub fn center(&self) -> TimeInt {
        self.min.midpoint(self.max)
    }

    #[inline]
    pub fn contains(&self, time: TimeInt) -> bool {
        self.min <= time && time <= self.max
    }

    /// Does this range fully contain the other?
    #[inline]
    pub fn contains_range(&self, other: Self) -> bool {
        self.min <= other.min && other.max <= self.max
    }

    #[inline]
    pub fn intersects(&self, other: Self) -> bool {
        self.min <= other.max && self.max >= other.min
    }

    #[inline]
    pub fn intersection(&self, other: Self) -> Option<Self> {
        self.intersects(other).then(|| Self {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        })
    }

    #[inline]
    pub fn union(&self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn from_relative_time_range(
        range: &re_types_core::datatypes::TimeRange,
        cursor: impl Into<re_types_core::datatypes::TimeInt>,
    ) -> Self {
        let cursor = cursor.into();

        let mut min = range.start.start_boundary_time(cursor);
        let mut max = range.end.end_boundary_time(cursor);

        if min > max {
            std::mem::swap(&mut min, &mut max);
        }

        Self::new(min, max)
    }
}

impl re_byte_size::SizeBytes for AbsoluteTimeRange {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

// ----------------------------------------------------------------------------

/// Like [`AbsoluteTimeRange`], but using [`TimeReal`] for improved precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AbsoluteTimeRangeF {
    pub min: TimeReal,
    pub max: TimeReal,
}

impl AbsoluteTimeRangeF {
    #[inline]
    pub fn new(min: impl Into<TimeReal>, max: impl Into<TimeReal>) -> Self {
        Self {
            min: min.into(),
            max: max.into(),
        }
    }

    #[inline]
    pub fn point(value: impl Into<TimeReal>) -> Self {
        let value = value.into();
        Self {
            min: value,
            max: value,
        }
    }

    /// Inclusive
    pub fn contains(&self, value: TimeReal) -> bool {
        self.min <= value && value <= self.max
    }

    /// Returns the point in the center of the range.
    pub fn center(&self) -> TimeReal {
        self.min.midpoint(self.max)
    }

    /// Where in the range is this value? Returns 0-1 if within the range.
    ///
    /// Returns <0 if before and >1 if after.
    pub fn inverse_lerp(&self, value: TimeReal) -> f64 {
        if self.min == self.max {
            0.5
        } else {
            (value - self.min).as_f64() / (self.max - self.min).as_f64()
        }
    }

    pub fn lerp(&self, t: f64) -> TimeReal {
        self.min + (self.max - self.min) * t
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.min == self.max
    }

    /// The amount of time or sequences covered by this range.
    #[inline]
    pub fn length(&self) -> TimeReal {
        self.max - self.min
    }

    /// Creates an [`AbsoluteTimeRange`] from self by rounding the start
    /// of the range down, and rounding the end of the range up.
    pub fn to_int(self) -> AbsoluteTimeRange {
        AbsoluteTimeRange::new(self.min.floor(), self.max.ceil())
    }

    /// In the given range, pick the "simplest" number.
    ///
    /// The simplest number is defined approximately as
    /// "the number with the most trailing zeroes".
    ///
    /// This is used in GUI code.
    ///
    /// NOTE: this function only cares about simple _integers_.
    /// We don't care about "smart" selection things smaller than a nanosecond or a single time step.
    /// So if both ends are within the same integer, then [`Self::center`] is returned.
    pub fn smart_aim(self) -> TimeReal {
        // Inspired by https://docs.rs/emath/latest/emath/smart_aim/fn.best_in_range_f64.html
        let Self { min, max } = self;
        if max < min {
            return Self::new(max, min).smart_aim();
        }
        if min == max {
            return min;
        }

        if min <= TimeReal::ZERO && TimeReal::ZERO <= max {
            return TimeReal::ZERO; // always prefer zero
        }
        if min < TimeReal::ZERO {
            // Keep things positive:
            return -Self::new(-max, -min).smart_aim();
        }

        let min = min.int();
        let max = max.int();

        if min == max {
            return self.center(); // We don't care
        }

        let min_str = min.to_string();
        let max_str = max.to_string();

        if min_str.len() < max_str.len() {
            // Different orders of magnitude.
            // Example: for `61` and `4236`: return 1000
            return TimeInt::new_temporal(10_i64.pow(max_str.len() as u32 - 1)).into();
        }

        debug_assert_eq!(min_str.len(), max_str.len());

        // We now have two positive integers of the same length.
        // We want to find the first non-matching digit,
        // which we will call the "deciding digit".
        // Everything before it will be the same,
        // everything after will be zero,
        // and the deciding digit itself will be picked as a "smart average"
        // min:    12345
        // max:    12780
        // output: 12500

        let len = min_str.len();
        for i in 0..len {
            if min_str.as_bytes()[i] != max_str.as_bytes()[i] {
                // Found the deciding digit at index `i`
                let prefix = &min_str[..i];
                let mut deciding_digit_min = min_str.as_bytes()[i];
                let deciding_digit_max = max_str.as_bytes()[i];

                debug_assert!(deciding_digit_min < deciding_digit_max);

                let rest_of_min_is_zeroes = min_str.as_bytes()[i + 1..].iter().all(|&c| c == b'0');

                if !rest_of_min_is_zeroes {
                    // There are more digits coming after `deciding_digit_min`, so we cannot pick it.
                    // So the true min of what we can pick is one greater:
                    deciding_digit_min += 1;
                }

                let deciding_digit = if deciding_digit_min == b'0' {
                    b'0'
                } else if deciding_digit_min <= b'5' && b'5' <= deciding_digit_max {
                    b'5' // 5 is the roundest number in the range
                } else {
                    deciding_digit_min.midpoint(deciding_digit_max)
                };

                let mut result_str = String::with_capacity(len);
                result_str.push_str(prefix);
                result_str.push(deciding_digit as char);
                for _ in i + 1..len {
                    result_str.push('0');
                }
                return TimeInt::new_temporal(
                    #[expect(clippy::unwrap_used)] // Cannot fail
                    re_format::parse_i64(&result_str).unwrap(),
                )
                .into();
            }
        }

        min.into() // All digits are the same
    }
}

impl From<AbsoluteTimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: AbsoluteTimeRangeF) -> Self {
        range.min..=range.max
    }
}

impl From<&AbsoluteTimeRangeF> for RangeInclusive<TimeReal> {
    fn from(range: &AbsoluteTimeRangeF) -> Self {
        range.min..=range.max
    }
}

impl From<AbsoluteTimeRange> for AbsoluteTimeRangeF {
    fn from(range: AbsoluteTimeRange) -> Self {
        Self::new(range.min, range.max)
    }
}

#[test]
fn test_smart_aim() {
    #[track_caller]
    fn test_f64((min, max): (f64, f64), expected: f64) {
        let range = AbsoluteTimeRangeF::new(TimeReal::from(min), TimeReal::from(max));
        let aimed = range.smart_aim().as_f64();
        assert!(
            aimed == expected,
            "smart_aim({min} – {max}) => {aimed}, but expected {expected}"
        );
    }
    #[track_caller]
    fn test_i64((min, max): (i64, i64), expected: i64) {
        let range = AbsoluteTimeRangeF::new(TimeReal::from(min), TimeReal::from(max));
        let aimed = range.smart_aim().as_f64();
        assert!(
            aimed == expected as f64,
            "smart_aim({min} – {max}) => {aimed}, but expected {expected}"
        );
    }

    test_i64((99, 300), 100);
    test_i64((300, 99), 100);
    test_i64((-99, -300), -100);
    test_i64((-99, 123), 0); // Prefer zero
    test_i64((4, 9), 5); // Prefer ending on 5
    test_i64((14, 19), 15); // Prefer ending on 5
    test_i64((12, 65), 50); // Prefer leading 5
    test_i64((493, 879), 500); // Prefer leading 5
    test_i64((37, 48), 40);
    test_i64((100, 123), 100);
    test_i64((101, 1000), 1000);
    test_i64((999, 1000), 1000);
    test_i64((123, 500), 500);
    test_i64((500, 777), 500);
    test_i64((500, 999), 500);
    test_i64((12345, 12780), 12500);
    test_i64((12371, 12376), 12375);
    test_i64((12371, 12376), 12375);

    test_f64((7.5, 16.3), 10.0);
    test_f64((7.5, 76.3), 10.0);
    test_f64((7.5, 763.3), 100.0);
    test_f64((7.5, 1_345.0), 1_000.0);
    test_f64((7.5, 123_456.0), 100_000.0);
    test_f64((-0.2, 0.0), 0.0); // Prefer zero
    test_f64((-10_004.23, 4.14), 0.0); // Prefer zero
    test_f64((-0.2, 100.0), 0.0); // Prefer zero
    test_f64((0.2, 0.0), 0.0); // Prefer zero
    test_f64((7.8, 17.8), 10.0);
    test_f64((14.1, 19.1), 15.0); // Prefer ending on 5
    test_f64((12.3, 65.9), 50.0); // Prefer leading 5

    test_f64((7.1, 7.6), 7.35); // NOTE: not simple, because we don't care about sub-integer precision in the smart-aim function atm
}
