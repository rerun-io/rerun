//! An integer range that always has a non-negative length.
//!
//! The standard [`std::ops::Range`] can have `start > end`
//! Taking a `Range` by argument thus means the callee must check for this eventuality and return an error.
//!
//! In contrast, [`Span`] always has a non-negative length, i.e. `len >= 0`.

use std::ops::{Mul, Range};

use num_traits::Unsigned;

/// An integer range who's length is always at least zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span<Idx: Unsigned + Copy> {
    /// The index of the first element.
    pub start: Idx,

    /// The number of elements in the range.
    pub len: Idx,
}

impl<Idx: Unsigned + Copy> Span<Idx> {
    /// The next element, just outside the range.
    #[inline]
    pub fn end(&self) -> Idx {
        self.start + self.len
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
}

impl Span<u32> {
    /// Widening cast; useful for indexing.
    #[inline]
    pub fn range_usize(self) -> Range<usize> {
        let Self { start, len } = self;
        Range {
            start: start as usize,
            end: start as usize + len as usize,
        }
    }
}

impl<Idx: Unsigned + Copy> From<Span<Idx>> for Range<Idx> {
    #[inline]
    fn from(value: Span<Idx>) -> Self {
        value.range()
    }
}

/// urange * scalar
impl<Idx: Unsigned + Copy + Mul> Mul<Idx> for Span<Idx> {
    type Output = Self;

    fn mul(self, rhs: Idx) -> Self::Output {
        let Self { start, len } = self;
        Self {
            start: rhs * start,
            len: rhs * len,
        }
    }
}
