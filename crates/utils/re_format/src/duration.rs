use itertools::Itertools as _;

use crate::{THIN_SPACE, format_uint};

/// How to round sub-seconds
enum Rounding {
    Closest,
    TowardsZero,
}

/// Format a duration as e.g. `3.2s` or `1h 42m`.
pub struct DurationFormatOptions {
    spaces: bool,
    only_seconds: bool,
    always_sign: bool,
    min_decimals: usize,
    max_decimals: usize,
    rounding: Rounding,
}

impl Default for DurationFormatOptions {
    fn default() -> Self {
        Self {
            spaces: true,
            only_seconds: true,
            always_sign: false,
            min_decimals: 1,
            max_decimals: 9,
            rounding: Rounding::Closest,
        }
    }
}

impl DurationFormatOptions {
    /// If true, insert spaces after units.
    pub fn with_spaces(mut self, spaces: bool) -> Self {
        self.spaces = spaces;
        self
    }

    /// If true, format 63 seconds as `63s`. If false, format it as `1m3s`
    pub fn with_only_seconds(mut self, only_seconds: bool) -> Self {
        self.only_seconds = only_seconds;
        self
    }

    /// Always show the sign, even if it is positive (`+`).
    #[inline]
    pub fn with_always_sign(mut self, always_sign: bool) -> Self {
        self.always_sign = always_sign;
        self
    }

    /// Number of sub-second decimals to at least show.
    ///
    /// So `3` means at least millisecond accuracy, etc.
    ///
    /// Supported values: 0,1,3,6,9
    pub fn with_min_decimals(mut self, min_decimals: usize) -> Self {
        debug_assert!(
            matches!(min_decimals, 0 | 1 | 3 | 6 | 9),
            "Expected min_decimals to be one of 0,1,3,6,9, but got {min_decimals}"
        );
        self.min_decimals = min_decimals;
        self.max_decimals = self.max_decimals.max(min_decimals);
        self
    }

    /// Number of sub-second to show at most.
    ///
    /// So `6` means at most microsecond accuracy, etc.
    ///
    /// Supported values: 0,1,3,6,9
    pub fn with_max_decimals(mut self, max_decimals: usize) -> Self {
        debug_assert!(
            matches!(max_decimals, 0 | 1 | 3 | 6 | 9),
            "Expected max_decimals to be one of 0,1,3,6,9, but got {max_decimals}"
        );
        self.max_decimals = max_decimals;
        self.min_decimals = self.min_decimals.min(max_decimals);
        self
    }

    /// When we hit `with_max_decimals`, round towards zero.
    pub fn round_towards_zero(mut self) -> Self {
        self.rounding = Rounding::TowardsZero;
        self
    }

    /// When we hit `with_max_decimals`, round to closest.
    pub fn round_to_closest(mut self) -> Self {
        self.rounding = Rounding::Closest;
        self
    }

    /// Formats nanoseconds in a pretty way, as specific.
    ///
    /// This function is NOT optimized for performance (and does many small allocations).
    pub fn format_nanos(self, ns: i64) -> String {
        const SEC_PER_MINUTE: u64 = 60;
        const SEC_PER_HOUR: u64 = 60 * SEC_PER_MINUTE;
        const SEC_PER_DAY: u64 = 24 * SEC_PER_HOUR;

        let Self {
            spaces,
            only_seconds,
            always_sign,
            mut min_decimals,
            max_decimals,
            rounding,
        } = self;

        let mut front = vec![];
        let ns = if ns < 0 {
            front.push(crate::MINUS.to_string());
            ns.unsigned_abs()
        } else {
            if always_sign {
                front.push('+'.to_string());
            }
            ns as u64
        };

        // The best way to approach this is by starting at the end (nanoseconds).
        // That way we can do proper rounding when needed.

        // The parts that make up the number, ordered back-to-front (ns, ms, us, …)
        let mut back_rev = vec![];

        let us = if 9 <= min_decimals || (9 <= max_decimals && !ns.is_multiple_of(1_000)) {
            min_decimals = 9; // make sure we include the next parts
            back_rev.push(format!("{:03}", ns % 1_000));
            back_rev.push(THIN_SPACE.to_string());
            ns / 1_000
        } else {
            match rounding {
                Rounding::Closest => (ns + 500) / 1_000,
                Rounding::TowardsZero => ns / 1_000,
            }
        };

        let ms = if 6 <= min_decimals || (6 <= max_decimals && !us.is_multiple_of(1_000)) {
            min_decimals = 6; // make sure we include the next parts
            back_rev.push(format!("{:03}", us % 1_000));
            back_rev.push(THIN_SPACE.to_string());
            us / 1_000
        } else {
            match rounding {
                Rounding::Closest => (us + 500) / 1_000,
                Rounding::TowardsZero => us / 1_000,
            }
        };

        // deca-seconds, i.e. tenths of a second
        let ds = if 3 <= min_decimals || (3 <= max_decimals && !ms.is_multiple_of(100)) {
            min_decimals = 3; // make sure we include the next parts
            back_rev.push(format!("{:02}", ms % 100));
            ms / 100
        } else {
            match rounding {
                Rounding::Closest => (ms + 50) / 100,
                Rounding::TowardsZero => ms / 100,
            }
        };

        let s = if 1 <= min_decimals || (1 <= max_decimals && !ds.is_multiple_of(10)) {
            back_rev.push(format!("{:01}", ds % 10));
            ds / 10
        } else {
            match rounding {
                Rounding::Closest => (ds + 5) / 10,
                Rounding::TowardsZero => ds / 10,
            }
        };

        if !back_rev.is_empty() {
            back_rev.push('.'.to_string());
        }

        if only_seconds {
            back_rev.push(format_uint(s));
            back_rev.insert(0, 's'.to_string());
        } else {
            let mut secs_remaining = s;
            let mut did_write = false;

            let days = secs_remaining / SEC_PER_DAY;
            if days > 0 {
                front.push(format!("{}d", format_uint(days)));
                secs_remaining -= days * SEC_PER_DAY;
                did_write = true;
            }

            let hours = secs_remaining / SEC_PER_HOUR;
            if hours > 0 {
                if did_write {
                    front.push(' '.to_string());
                }
                front.push(format!("{hours}h"));
                secs_remaining -= hours * SEC_PER_HOUR;
                did_write = true;
            }

            let minutes = secs_remaining / SEC_PER_MINUTE;
            if minutes > 0 {
                if spaces && did_write {
                    front.push(' '.to_string());
                }
                front.push(format!("{minutes}m"));
                secs_remaining -= minutes * SEC_PER_MINUTE;
                did_write = true;
            }

            if secs_remaining > 0 || !back_rev.is_empty() || !did_write {
                if spaces && did_write {
                    front.push(' '.to_string());
                }
                back_rev.push(format_uint(secs_remaining));
                back_rev.insert(0, 's'.to_string());
            }
        }

        itertools::chain!(front, back_rev.into_iter().rev()).join("")
    }
}

#[test]
fn test_format_duration() {
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(9)
            .with_only_seconds(true)
            .format_nanos(59_123_456_789),
        "59.123 456 789s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(6)
            .with_only_seconds(true)
            .format_nanos(59_123_456_789),
        "59.123 457s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(3)
            .with_only_seconds(true)
            .format_nanos(59_123_456_789),
        "59.123s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(1)
            .with_only_seconds(true)
            .format_nanos(59_123_456_789),
        "59.1s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(0)
            .with_only_seconds(true)
            .format_nanos(59_123_456_789),
        "59s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(0)
            .with_only_seconds(false)
            .format_nanos(59_123_456_789),
        "59s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(1)
            .with_only_seconds(false)
            .format_nanos(59_123_456_789),
        "59.1s"
    );

    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(0)
            .with_only_seconds(true)
            .format_nanos(59_999_999_987),
        "60s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(9)
            .with_only_seconds(true)
            .format_nanos(59_999_999_987),
        "59.999 999 987s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_max_decimals(0)
            .with_only_seconds(false)
            .format_nanos(59_999_999_987),
        "1m"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_min_decimals(1)
            .with_max_decimals(6)
            .with_only_seconds(false)
            .format_nanos(59_999_999_987),
        "1m 0.0s"
    );
    assert_eq!(
        DurationFormatOptions::default()
            .with_min_decimals(1)
            .with_max_decimals(6)
            .with_only_seconds(false)
            .round_towards_zero()
            .format_nanos(59_999_999_987),
        "59.999 999s"
    );

    fn format_as_secs(nanos: i64) -> String {
        DurationFormatOptions::default()
            .with_min_decimals(0)
            .with_max_decimals(6)
            .format_nanos(nanos)
    }

    assert_eq!(format_as_secs(0), "0s");
    assert_eq!(format_as_secs(1_000), "0.000 001s");
    assert_eq!(format_as_secs(2_000_000), "0.002s");
    assert_eq!(format_as_secs(1_200_300_400_500_789), "1 200 300.400 501s");
    assert_eq!(format_as_secs(1_200_300_000_000_000), "1 200 300s");
    assert_eq!(format_as_secs(12_000_000_000), "12s");
    assert_eq!(format_as_secs(12_100_000_000), "12.1s");
    assert_eq!(format_as_secs(12_120_000_000), "12.120s");
    assert_eq!(format_as_secs(12_120_340_001), "12.120 340s");
    assert_eq!(format_as_secs(12_100_000_001), "12.1s");
}
