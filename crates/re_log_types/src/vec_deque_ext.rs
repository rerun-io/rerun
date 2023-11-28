use std::collections::VecDeque;

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

    assert!(v.is_sorted()); // !

    v.extend([7, 6, 5]);
    assert!(!v.is_sorted());

    v.sort();
    assert!(v.is_sorted());
}

// ---

/// Extends [`VecDeque`] with extra removal routines.
pub trait VecDequeRemovalExt<T> {
    /// Removes an element from anywhere in the deque and returns it, replacing it with
    /// whichever end element that this is closer to the removal point.
    ///
    /// If `index` points to the front or back of the queue, the removal is guaranteed to preserve
    /// ordering; otherwise it doesn not.
    /// In either case, this is *O*(1).
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// Element at index 0 is the front of the queue.
    fn swap_remove(&mut self, index: usize) -> Option<T>;
}

impl<T: Clone> VecDequeRemovalExt<T> for VecDeque<T> {
    #[inline]
    fn swap_remove(&mut self, index: usize) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        if index == 0 {
            let v = self.get(0).cloned();
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
