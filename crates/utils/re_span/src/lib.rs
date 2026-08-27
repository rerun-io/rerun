//! An integer range that always has a non-negative length.
//!
//! The standard [`std::ops::Range`] can have `start > end`
//! Taking a `Range` by argument thus means the callee must check for this eventuality and return an error.
//!
//! In contrast, [`Span`] always has a non-negative length, i.e. `len >= 0`.

use std::ops::Range;

use num_traits::{CheckedAdd, SaturatingAdd, SaturatingSub, Unsigned};

/// An integer range who's length is always at least zero.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span<Idx: Unsigned + Copy> {
    /// The index of the first element.
    pub start: Idx,

    /// The number of elements in the range.
    pub len: Idx,
}

impl<Idx: Unsigned + Copy> Span<Idx> {
    /// Construct from `start` and `len`.
    #[inline]
    pub const fn from_start_len(start: Idx, len: Idx) -> Self {
        Self { start, len }
    }

    /// Construct from `start` (inclusive) and `end` (exclusive).
    ///
    /// See also [`Self::try_from_start_end`].
    ///
    /// # Panics
    /// Panics if `end < start`.
    #[inline]
    pub fn from_start_end(start: Idx, end: Idx) -> Self
    where
        Idx: PartialOrd,
    {
        assert!(start <= end, "Span start must be less than or equal to end");

        Self {
            start,
            len: end - start,
        }
    }

    /// Construct from `start` (inclusive) and `end` (exclusive).
    ///
    /// Returns `None` if `end < start`.
    #[inline]
    pub fn try_from_start_end(start: Idx, end: Idx) -> Option<Self>
    where
        Idx: PartialOrd,
    {
        (start <= end).then(|| Self {
            start,
            len: end - start,
        })
    }

    /// The next element, just outside the range.
    #[inline]
    pub fn end(&self) -> Idx {
        self.start + self.len
    }

    /// Is the span empty, i.e. has zero length?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len.is_zero()
    }

    /// Is the given index within the span?
    #[inline]
    pub fn contains(&self, idx: Idx) -> bool
    where
        Idx: PartialOrd,
    {
        self.start <= idx && idx < self.end()
    }

    /// Useful when slicing a slice
    #[inline]
    pub fn range(self) -> Range<Idx> {
        let Self { start, len } = self;
        Range {
            start,
            end: start + len,
        }
    }

    pub fn try_cast<Narrow>(self) -> Option<Span<Narrow>>
    where
        Narrow: TryFrom<Idx> + Unsigned + Copy,
    {
        Some(Span {
            start: self.start.try_into().ok()?,
            len: self.len.try_into().ok()?,
        })
    }

    /// The smallest span covering both `self` and `other`, including any gap between them.
    #[inline]
    pub fn union(self, other: Self) -> Self
    where
        Idx: Ord,
    {
        let start = self.start.min(other.start);
        let end = self.end().max(other.end());
        Self {
            start,
            len: end - start,
        }
    }

    /// Clamp the span so it fits inside a container of the given length, i.e. within `0..len`.
    ///
    /// The result is empty if the span starts at or beyond `len`.
    #[inline]
    pub fn clamped_to(self, len: Idx) -> Self
    where
        Idx: Ord,
    {
        let start = self.start.min(len);
        Self {
            start,
            len: self.len.min(len - start),
        }
    }

    /// Shift the span up by `rhs`, keeping its length.
    ///
    /// Overflows like normal unsigned addition if `self.end() + rhs` exceeds the maximum;
    /// see [`Self::saturating_add`] for a clamping version.
    #[inline]
    #[must_use]
    #[expect(clippy::should_implement_trait)]
    pub fn add(self, rhs: Idx) -> Self {
        let Self { start, len } = self;
        Self {
            start: start + rhs,
            len,
        }
    }

    /// Shift the span down by `rhs`, keeping its length.
    ///
    /// Underflows like normal unsigned subtraction if `rhs > start`;
    /// see [`Self::saturating_sub`] for a clamping version.
    #[inline]
    #[must_use]
    #[expect(clippy::should_implement_trait)]
    pub fn sub(self, rhs: Idx) -> Self {
        let Self { start, len } = self;
        Self {
            start: start - rhs,
            len,
        }
    }

    /// Multiply both `start` and `len` by `scale`.
    ///
    /// Useful for translating an element-span into a byte-span,
    /// by scaling with `size_of::<T>()`.
    #[inline]
    #[must_use]
    pub fn scale(self, scale: Idx) -> Self {
        let Self { start, len } = self;
        Self {
            start: scale * start,
            len: scale * len,
        }
    }

    /// Shift the span up by `rhs`, clamping both endpoints at the maximum value.
    ///
    /// The length shrinks if the span crosses the maximum:
    /// for `u8`, `(250..254).saturating_add(3) == 253..255`.
    #[inline]
    pub fn saturating_add(self, rhs: Idx) -> Self
    where
        Idx: SaturatingAdd,
    {
        let start = self.start.saturating_add(&rhs);
        let end = self.start.saturating_add(&self.len).saturating_add(&rhs);
        Self {
            start,
            len: end - start,
        }
    }

    /// Shift the span down by `rhs`, clamping both endpoints at zero.
    ///
    /// The length shrinks if the span crosses zero:
    /// `(2..5).saturating_sub(3) == 0..2`.
    #[inline]
    pub fn saturating_sub(self, rhs: Idx) -> Self
    where
        Idx: SaturatingSub,
    {
        let start = self.start.saturating_sub(&rhs);
        let end = self.end().saturating_sub(&rhs);
        Self {
            start,
            len: end - start,
        }
    }
}

impl Span<u32> {
    /// Widening cast; useful for indexing.
    #[inline]
    pub const fn range_usize(self) -> Range<usize> {
        let Self { start, len } = self;
        Range {
            start: start as usize,
            end: start as usize + len as usize,
        }
    }
}

impl Span<usize> {
    /// Widening cast.
    #[inline]
    pub const fn cast_u64(self) -> Span<u64> {
        let Self { start, len } = self;
        Span {
            start: start as u64,
            len: len as u64,
        }
    }
}

impl Span<u64> {
    /// Cast to native pointer width; useful for indexing on native platforms.
    #[inline]
    pub const fn range_usize(self) -> Range<usize> {
        let Self { start, len } = self;
        Range {
            start: start as usize,
            end: start as usize + len as usize,
        }
    }
}

/// Iterate over the indices of the span.
///
/// Implemented per concrete index type because the underlying
/// [`Range`] iterator requires the unstable `Step` trait.
macro_rules! impl_into_iterator {
    ($($idx:ty),*) => {
        $(
            impl IntoIterator for Span<$idx> {
                type Item = $idx;
                type IntoIter = Range<$idx>;

                #[inline]
                fn into_iter(self) -> Self::IntoIter {
                    self.range()
                }
            }
        )*
    };
}

impl_into_iterator!(u8, u16, u32, u64, usize);

/// Formats like the equivalent [`Range`], e.g. `3..7`.
impl<Idx: Unsigned + Copy + CheckedAdd + std::fmt::Debug> std::fmt::Debug for Span<Idx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { start, len } = *self;
        match start.checked_add(&len) {
            Some(end) => write!(f, "{start:?}..{end:?}"),
            None => write!(f, "{start:?}..{start:?}+{len:?} (overflow)"),
        }
    }
}

impl<Idx: Unsigned + Copy> From<Span<Idx>> for Range<Idx> {
    #[inline]
    fn from(value: Span<Idx>) -> Self {
        value.range()
    }
}

impl<Idx: Unsigned + Copy> From<Span<Idx>> for core::range::Range<Idx> {
    #[inline]
    fn from(value: Span<Idx>) -> Self {
        let Range { start, end } = value.range();
        Self { start, end }
    }
}

#[cfg(test)]
mod tests {
    use super::Span;

    #[test]
    fn try_from_start_end_rejects_inverted_ranges() {
        assert_eq!(
            Span::try_from_start_end(3_u64, 7),
            Some(Span::from_start_len(3, 4))
        );
        assert_eq!(
            Span::try_from_start_end(5_u64, 5),
            Some(Span::from_start_len(5, 0))
        );
        assert_eq!(Span::try_from_start_end(7_u64, 3), None);
    }

    #[test]
    fn union_covers_both_spans_and_the_gap() {
        assert_eq!(
            Span::from_start_len(2_u64, 3).union(Span::from_start_len(10, 2)),
            Span::from_start_len(2, 10)
        );
        assert_eq!(
            Span::from_start_len(2_u64, 10).union(Span::from_start_len(4, 2)),
            Span::from_start_len(2, 10)
        );
        assert_eq!(
            Span::from_start_len(5_u64, 0).union(Span::from_start_len(5, 0)),
            Span::from_start_len(5, 0)
        );
    }

    #[test]
    fn clamped_to_caps_both_endpoints() {
        assert_eq!(
            Span::from_start_len(2_u64, 3).clamped_to(10),
            Span::from_start_len(2, 3)
        );
        assert_eq!(
            Span::from_start_len(2_u64, 30).clamped_to(10),
            Span::from_start_len(2, 8)
        );
        assert_eq!(
            Span::from_start_len(10_u64, 3).clamped_to(10),
            Span::from_start_len(10, 0)
        );
        assert_eq!(
            Span::from_start_len(20_u64, 3).clamped_to(10),
            Span::from_start_len(10, 0)
        );
    }

    #[test]
    fn saturating_add_clamps_at_the_maximum() {
        assert_eq!(
            Span::from_start_len(2_u8, 3).saturating_add(1),
            Span::from_start_len(3, 3)
        );
        assert_eq!(
            Span::from_start_len(250_u8, 4).saturating_add(3),
            Span::from_start_len(253, 2)
        );
        assert_eq!(
            Span::from_start_len(250_u8, 4).saturating_add(200),
            Span::from_start_len(255, 0)
        );
    }

    #[test]
    fn debug_does_not_panic_on_overflowing_spans() {
        assert_eq!(format!("{:?}", Span::from_start_len(3_u8, 4)), "3..7");
        assert_eq!(
            format!("{:?}", Span::from_start_len(200_u8, 100)),
            "200..200+100 (overflow)"
        );
    }
}
