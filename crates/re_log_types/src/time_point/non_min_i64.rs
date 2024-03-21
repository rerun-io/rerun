// Adapted from <https://github.com/LPGhatguy/nonmax>.
//
// Copyright (c) 2020 Lucien Greathouse | MIT or Apache 2

// We need unsafety in order to hijack `NonZeroI64` for our purposes.
#![allow(
    unsafe_code,
    clippy::undocumented_unsafe_blocks,
    unsafe_op_in_unsafe_fn
)]

// ---

/// An error type returned when a checked integral type conversion fails (mimics [`std::num::TryFromIntError`])
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromIntError;

impl std::error::Error for TryFromIntError {}

impl core::fmt::Display for TryFromIntError {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        "out of range integral type conversion attempted".fmt(fmt)
    }
}

impl From<core::num::TryFromIntError> for TryFromIntError {
    fn from(_: core::num::TryFromIntError) -> Self {
        Self
    }
}

impl From<core::convert::Infallible> for TryFromIntError {
    fn from(never: core::convert::Infallible) -> Self {
        match never {}
    }
}

/// ---

/// An integer that is known not to equal its minimum value.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NonMinI64(core::num::NonZeroI64);

impl PartialOrd for NonMinI64 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NonMinI64 {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}

impl NonMinI64 {
    pub const ZERO: NonMinI64 = unsafe { Self::new_unchecked(0) };
    pub const ONE: NonMinI64 = unsafe { Self::new_unchecked(1) };
    pub const MIN: NonMinI64 = unsafe { Self::new_unchecked(i64::MIN + 1) };
    pub const MAX: NonMinI64 = unsafe { Self::new_unchecked(i64::MAX) };

    /// Creates a new non-min if the given value is not the minimum value.
    #[inline]
    pub const fn new(value: i64) -> Option<Self> {
        match core::num::NonZeroI64::new(value ^ i64::MIN) {
            None => None,
            Some(value) => Some(Self(value)),
        }
    }

    /// Creates a new non-min without checking the value.
    ///
    /// # Safety
    ///
    /// The value must not equal the minimum representable value for the
    /// primitive type.
    #[inline]
    pub const unsafe fn new_unchecked(value: i64) -> Self {
        let inner = core::num::NonZeroI64::new_unchecked(value ^ i64::MIN);
        Self(inner)
    }

    /// Returns the value as a primitive type.
    #[inline]
    pub const fn get(&self) -> i64 {
        self.0.get() ^ i64::MIN
    }
}

impl Default for NonMinI64 {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<NonMinI64> for i64 {
    #[inline]
    fn from(value: NonMinI64) -> Self {
        value.get()
    }
}

impl core::convert::TryFrom<i64> for NonMinI64 {
    type Error = TryFromIntError;

    #[inline]
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(TryFromIntError)
    }
}

impl core::ops::BitAnd<NonMinI64> for NonMinI64 {
    type Output = NonMinI64;

    #[inline]
    fn bitand(self, rhs: NonMinI64) -> Self::Output {
        unsafe { NonMinI64::new_unchecked(self.get() & rhs.get()) }
    }
}

impl core::ops::BitAndAssign<NonMinI64> for NonMinI64 {
    #[inline]
    fn bitand_assign(&mut self, rhs: NonMinI64) {
        *self = *self & rhs;
    }
}

impl core::fmt::Debug for NonMinI64 {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.get(), f)
    }
}

impl core::fmt::Display for NonMinI64 {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.get(), f)
    }
}

impl core::fmt::Binary for NonMinI64 {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Binary::fmt(&self.get(), f)
    }
}

impl core::fmt::Octal for NonMinI64 {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Octal::fmt(&self.get(), f)
    }
}

impl core::fmt::LowerHex for NonMinI64 {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(&self.get(), f)
    }
}

impl core::fmt::UpperHex for NonMinI64 {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::UpperHex::fmt(&self.get(), f)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NonMinI64 {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.get().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for NonMinI64 {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<NonMinI64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = i64::deserialize(deserializer)?;
        Self::try_from(value).map_err(serde::de::Error::custom)
    }
}
