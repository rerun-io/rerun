//! A histogram with `i64` keys and `u32` counts, supporting both sparse and dense uses.
//!
//! It supports high-level summaries of the histogram, so that you can quickly
//! get a birds-eye view of the data without having to visit every point in the histogram.
//!
//! You can also think of the histogram as a multi-set,
//! where you can insert the same key multiple times and then query
//! how many times you've inserted it.

mod tree;

pub use tree::{Int64Histogram, Iter};

// -----------------------------------------------------------------------------------

/// We use `u64` keys in the internal structures,
/// because it is so much easier to work with
pub(crate) fn u64_key_from_i64_key(key: i64) -> u64 {
    // key.wrapping_add_unsigned(i64::MIN.unsigned_abs()) // unstable
    (key as i128 + i64::MAX as i128 + 1) as _
}

pub(crate) fn i64_key_from_u64_key(key: u64) -> i64 {
    (key as i128 + i64::MIN as i128) as _
}

#[test]
fn test_u64_i64_key_conversions() {
    assert_eq!(u64_key_from_i64_key(i64::MIN), u64::MIN);
    assert_eq!(u64_key_from_i64_key(i64::MIN + 1), 1);
    assert_eq!(u64_key_from_i64_key(i64::MIN + 2), 2);
    assert_eq!(u64_key_from_i64_key(i64::MAX - 2), u64::MAX - 2);
    assert_eq!(u64_key_from_i64_key(i64::MAX - 1), u64::MAX - 1);
    assert_eq!(u64_key_from_i64_key(i64::MAX), u64::MAX);

    assert_eq!(i64_key_from_u64_key(u64::MIN), i64::MIN);
    assert_eq!(i64_key_from_u64_key(1), i64::MIN + 1);
    assert_eq!(i64_key_from_u64_key(2), i64::MIN + 2);
    assert_eq!(i64_key_from_u64_key(u64::MAX - 2), i64::MAX - 2);
    assert_eq!(i64_key_from_u64_key(u64::MAX - 1), i64::MAX - 1);
    assert_eq!(i64_key_from_u64_key(u64::MAX), i64::MAX);
}

// -----------------------------------------------------------------------------------

/// An inclusive range.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RangeU64 {
    /// inclusive
    pub min: u64,

    /// inclusive
    pub max: u64,
}

impl RangeU64 {
    pub fn new(min: u64, max: u64) -> Self {
        Self { min, max }
    }

    pub fn single(value: u64) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    #[inline]
    pub fn contains(&self, value: u64) -> bool {
        self.min <= value && value <= self.max
    }

    #[inline]
    pub fn contains_all_of(&self, other: Self) -> bool {
        self.contains(other.min) && self.contains(other.max)
    }

    #[inline]
    pub fn intersects(&self, other: Self) -> bool {
        self.min <= other.max && other.min <= self.max
    }
}

impl std::fmt::Debug for RangeU64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RangeU64[{}, {}]", self.min, self.max)
    }
}

// -----------------------------------------------------------------------------------

/// An inclusive range.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RangeI64 {
    /// inclusive
    pub min: i64,

    /// inclusive
    pub max: i64,
}

impl RangeI64 {
    pub fn new(min: i64, max: i64) -> Self {
        Self { min, max }
    }

    pub fn single(value: i64) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    #[inline]
    pub fn contains(&self, value: i64) -> bool {
        self.min <= value && value <= self.max
    }

    #[inline]
    pub fn length(&self) -> u64 {
        (self.max - self.min + 1) as u64
    }
}

impl std::fmt::Debug for RangeI64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RangeI64[{}, {}]", self.min, self.max)
    }
}
