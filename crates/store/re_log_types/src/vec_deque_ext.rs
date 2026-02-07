use std::collections::VecDeque;
use std::ops::Range;

// --
// ---

/// Extends [`VecDeque`] with extra sorting routines.
pub trait VecDequeSortingExt<T> {
    /// Sorts `self`.
    ///
    /// Makes sure to render `self` contiguous first, if needed.
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

    v.extend([7, 6, 5]);
    assert!(!v.is_sorted());

    v.sort();
    assert!(v.is_sorted());
}

// ---

/// Extends [`VecDeque`] with extra insertion routines.
pub trait VecDequeInsertionExt<T> {
    /// Inserts multiple elements at `index` within the deque, shifting all elements
    /// with indices greater than or equal to `index` towards the back.
    ///
    /// This is O(1) if `index` corresponds to either the start or the end of the deque.
    /// Otherwise, this means splitting the deque into two pieces then stitching them back together
    /// at both ends of the added data.
    ///
    /// Panics if `index` is out of bounds.
    fn insert_many(&mut self, index: usize, values: impl ExactSizeIterator<Item = T>);
}

impl<T> VecDequeInsertionExt<T> for VecDeque<T> {
    fn insert_many(&mut self, index: usize, values: impl ExactSizeIterator<Item = T>) {
        if index == self.len() {
            self.extend(values); // has a specialization fast-path builtin
        } else if index == 0 {
            let n = values.len();
            self.extend(values);
            self.rotate_right(n);
        } else {
            let right = self.split_off(index);

            // NOTE: definitely more elegant, but _much_ slower :(
            // self.extend(values);
            // self.extend(right);

            *self = std::mem::take(self)
                .into_iter()
                .chain(values)
                .chain(right)
                .collect();
        }
    }
}

#[test]
fn insert_many() {
    let mut v: VecDeque<i64> = vec![].into();

    assert!(v.is_empty());

    v.insert_many(0, [1, 2, 3].into_iter());
    assert_deque_eq([1, 2, 3], v.clone());

    v.insert_many(0, [4, 5].into_iter());
    assert_deque_eq([4, 5, 1, 2, 3], v.clone());

    v.insert_many(2, std::iter::once(6));
    assert_deque_eq([4, 5, 6, 1, 2, 3], v.clone());

    v.insert_many(v.len(), [7, 8, 9, 10].into_iter());
    assert_deque_eq([4, 5, 6, 1, 2, 3, 7, 8, 9, 10], v.clone());

    v.insert_many(5, [11, 12].into_iter());
    assert_deque_eq([4, 5, 6, 1, 2, 11, 12, 3, 7, 8, 9, 10], v.clone());
}

// ---

/// Extends [`VecDeque`] with extra removal routines.
pub trait VecDequeRemovalExt<T> {
    /// Removes an element from anywhere in the deque and returns it, replacing it with
    /// whichever end element that this is closer to the removal point.
    ///
    /// If `index` points to the front or back of the queue, the removal is guaranteed to preserve
    /// ordering; otherwise it doesn't.
    /// In either case, this is *O*(1).
    ///
    /// Returns `None` if `index` is out of bounds.
    fn swap_remove(&mut self, index: usize) -> Option<T>;

    /// Splits the deque into two at the given index.
    ///
    /// Returns a newly allocated `VecDeque`. `self` contains elements `[0, at)`,
    /// and the returned deque contains elements `[at, len)`.
    ///
    /// If `at` is equal or greater than the length, the returned `VecDeque` is empty.
    ///
    /// Note that the capacity of `self` does not change.
    fn split_off_or_default(&mut self, at: usize) -> Self;

    /// Removes and returns the elements in the given `range` from the deque.
    ///
    /// This is O(1) if `range` either starts at the beginning of the deque, or ends at the end of
    /// the deque, or both.
    /// Otherwise, this means splitting the deque into three pieces, dropping the middle one, then
    /// stitching back the remaining two.
    ///
    /// This doesn't do any kind of element re-ordering: if the deque was sorted before, it's
    /// still sorted after.
    ///
    /// Panics if `index` is out of bounds.
    //
    // NOTE: We take a `Range` rather than a `impl RangeBounds` because we rely on the fact that
    // `range` must be contiguous.
    fn remove_range(&mut self, range: Range<usize>);
}

impl<T: Clone> VecDequeRemovalExt<T> for VecDeque<T> {
    #[inline]
    fn swap_remove(&mut self, index: usize) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        if index == 0 {
            let v = self.front().cloned();
            self.rotate_left(1);
            self.truncate(self.len() - 1);
            v
        } else if index + 1 == self.len() {
            let v = self.get(index).cloned();
            self.truncate(self.len() - 1);
            v
        } else if index < self.len() / 2 {
            self.swap_remove_front(index)
        } else {
            self.swap_remove_back(index)
        }
    }

    #[inline]
    fn split_off_or_default(&mut self, at: usize) -> Self {
        if at >= self.len() {
            return Default::default();
        }
        self.split_off(at)
    }

    #[inline]
    fn remove_range(&mut self, range: Range<usize>) {
        if range.start == 0 && range.end == self.len() {
            self.clear();
        } else if range.start == 0 {
            self.rotate_left(range.len());
            self.truncate(self.len() - range.len());
        } else if range.end == self.len() {
            self.truncate(self.len() - range.len());
        } else {
            // NOTE: More elegant, but also 70% slower (!)
            // let mid_and_right = self.split_off(range.start);
            // self.extend(mid_and_right.into_iter().skip(range.len()));

            let mut mid_and_right = self.split_off(range.start);
            mid_and_right.rotate_left(range.len());
            mid_and_right.truncate(mid_and_right.len() - range.len());
            self.extend(mid_and_right);
        }
    }
}

#[test]
fn swap_remove() {
    let mut v: VecDeque<i64> = vec![].into();

    assert!(v.swap_remove(0).is_none());
    assert!(v.is_sorted());

    v.push_front(1);
    assert!(v.is_sorted());

    assert!(v.swap_remove(1).is_none());
    assert_eq!(Some(1), v.swap_remove(0));
    assert!(v.is_sorted());

    v.extend([4, 5, 6, 7]);
    assert!(v.is_sorted());

    assert_eq!(Some(4), v.swap_remove(0));
    assert!(v.is_sorted());

    assert_eq!(Some(7), v.swap_remove(2));
    assert!(v.is_sorted());

    assert_eq!(Some(6), v.swap_remove(1));
    assert!(v.is_sorted());

    assert_eq!(Some(5), v.swap_remove(0));
    assert!(v.is_sorted());
}

#[test]
fn remove_range() {
    let mut v: VecDeque<i64> = vec![].into();

    assert!(v.is_empty());
    assert!(v.is_sorted());

    v.insert_many(0, [1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter());
    assert_deque_eq([1, 2, 3, 4, 5, 6, 7, 8, 9], v.clone());
    assert!(v.is_sorted());

    {
        let mut v = v.clone();
        v.remove_range(0..v.len());
        assert!(v.is_empty());
        assert!(v.is_sorted());
    }

    v.remove_range(0..2);
    assert_deque_eq([3, 4, 5, 6, 7, 8, 9], v.clone());
    assert!(v.is_sorted());

    v.remove_range(v.len() - 2..v.len());
    assert_deque_eq([3, 4, 5, 6, 7], v.clone());
    assert!(v.is_sorted());

    v.remove_range(1..v.len() - 1);
    assert_deque_eq([3, 7], v.clone());
    assert!(v.is_sorted());

    v.remove_range(0..1);
    assert_deque_eq([7], v.clone());
    assert!(v.is_sorted());

    v.remove_range(0..1);
    assert_deque_eq([], v.clone());
    assert!(v.is_sorted());
}

// ---

#[cfg(test)]
fn assert_deque_eq(expected: impl IntoIterator<Item = i64>, got: impl IntoIterator<Item = i64>) {
    let expected = expected.into_iter().collect::<Vec<_>>();
    let got = got.into_iter().collect::<Vec<_>>();
    similar_asserts::assert_eq!(expected, got);
}
