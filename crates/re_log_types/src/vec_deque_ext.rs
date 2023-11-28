use std::collections::VecDeque;

// ---

/// Extends [`VecDeque`] with extra sorting routines.
pub trait VecDequeSortingExt<T> {
    /// Sorts `self`.
    ///
    /// Makes sure to render `self` contigous first, if needed.
    fn sort(&mut self);

    /// Check whether `self` is sorted.
    ///
    /// `self` doesn't need to be contiguous.
    fn is_sorted(&self) -> bool;
}

impl<T: Clone + PartialOrd + Ord> VecDequeSortingExt<T> for VecDeque<T> {
    #[inline]
    fn sort(&mut self) {
        self.make_contiguous();
        let (values, &mut []) = self.as_mut_slices() else {
            unreachable!();
        };
        values.sort();
    }

    #[inline]
    fn is_sorted(&self) -> bool {
        if self.is_empty() {
            return true;
        }

        let (left, right) = self.as_slices();

        let left_before_right = || {
            if let (Some(left_last), Some(right_first)) = (left.last(), right.first()) {
                left_last <= right_first
            } else {
                true
            }
        };
        let left_is_sorted = || !left.windows(2).any(|values| values[0] > values[1]);
        let right_is_sorted = || !right.windows(2).any(|values| values[0] > values[1]);

        left_before_right() && left_is_sorted() && right_is_sorted()
    }
}

#[test]
fn is_sorted() {
    let mut v: VecDeque<i64> = vec![].into();

    assert!(v.is_sorted());

    v.extend([1, 2, 3]);
    assert!(v.is_sorted());

    v.push_front(4);
    assert!(!v.is_sorted());

    v.rotate_left(1);
    assert!(v.is_sorted());

    assert!(v.is_sorted()); // !

    v.extend([7, 6, 5]);
    assert!(!v.is_sorted());

    v.sort();
    assert!(v.is_sorted());
}
