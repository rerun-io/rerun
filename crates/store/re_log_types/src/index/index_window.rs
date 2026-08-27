use crate::{AbsoluteTimeRange, TimeInt};

/// A symmetric window around a point on an index, as an inclusive half-width in that index's own units.
///
/// Nanoseconds on a time index, ticks on a sequence index — the same units [`TimeInt`] itself spans.
/// Unsigned, since a window has no direction: it is applied to both sides of the point.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes)]
pub struct IndexWindow(u64);

impl IndexWindow {
    /// Admits only values at the point itself.
    pub const ZERO: Self = Self(0);

    #[inline]
    pub const fn new(half_width: u64) -> Self {
        Self(half_width)
    }

    /// The half-width, for the wire and for display.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// The earliest index value this window admits around `at`, saturating at [`TimeInt::MIN`].
    #[inline]
    pub fn before(self, at: TimeInt) -> TimeInt {
        at.saturating_sub(self.to_i64())
    }

    /// The latest index value this window admits around `at`, saturating at [`TimeInt::MAX`].
    #[inline]
    pub fn after(self, at: TimeInt) -> TimeInt {
        at.saturating_add(self.to_i64())
    }

    /// Everything this window admits around `at`, both bounds inclusive.
    #[inline]
    pub fn around(self, at: TimeInt) -> AbsoluteTimeRange {
        AbsoluteTimeRange::new(self.before(at), self.after(at))
    }

    /// Saturating cast to `i64`, so a window wider than the index itself cannot wrap.
    #[inline]
    fn to_i64(self) -> i64 {
        i64::try_from(self.0).unwrap_or(i64::MAX)
    }
}

impl std::fmt::Display for IndexWindow {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "±{}", self.0)
    }
}

impl From<u64> for IndexWindow {
    #[inline]
    fn from(half_width: u64) -> Self {
        Self::new(half_width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_saturates_rather_than_wrapping() {
        let huge = IndexWindow::new(u64::MAX);
        assert_eq!(TimeInt::MIN, huge.before(TimeInt::new_temporal(0)));
        assert_eq!(TimeInt::MAX, huge.after(TimeInt::new_temporal(0)));

        assert_eq!(
            TimeInt::MAX,
            IndexWindow::new(1).after(TimeInt::MAX),
            "one past the end is still the end"
        );
    }

    #[test]
    fn a_zero_window_admits_only_the_point_itself() {
        let at = TimeInt::new_temporal(10);
        let window = IndexWindow::ZERO.around(at);
        assert!(window.contains(at));
        assert!(!window.contains(TimeInt::new_temporal(11)));
        assert!(!window.contains(TimeInt::new_temporal(9)));
    }

    /// A window is a temporal notion, so it leaves [`TimeInt::STATIC`] alone rather than dragging
    /// it onto the timeline.
    #[test]
    fn a_window_around_static_stays_static() {
        let window = IndexWindow::new(5);
        assert_eq!(TimeInt::STATIC, window.before(TimeInt::STATIC));
        assert_eq!(TimeInt::STATIC, window.after(TimeInt::STATIC));
    }

    #[test]
    fn both_bounds_are_inclusive() {
        let window = IndexWindow::new(5);
        let at = TimeInt::new_temporal(20);

        assert_eq!(TimeInt::new_temporal(15), window.before(at));
        assert_eq!(TimeInt::new_temporal(25), window.after(at));
        assert_eq!(window.around(at), AbsoluteTimeRange::new(15, 25));

        assert!(window.around(at).contains(TimeInt::new_temporal(15)));
        assert!(window.around(at).contains(TimeInt::new_temporal(25)));
        assert!(!window.around(at).contains(TimeInt::new_temporal(26)));
    }
}
