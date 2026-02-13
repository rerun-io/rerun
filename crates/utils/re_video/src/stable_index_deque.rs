use std::collections::VecDeque;

/// A deque with an offset that allows indices into it to stay valid.
///
/// Does not expose the underlying [`VecDeque`] directly to guarantee that all operations are valid.
/// This is useful to use stable indices for addressing in a growing/shrinking deque
/// without having to resort to a more complex map datastructure.
///
/// To illustrate:
/// ```
/// // Indices for deque are unstable:
/// # use std::collections::VecDeque;
/// let mut v = (0..2).collect::<VecDeque<i32>>();
/// assert_eq!(v.get(1), Some(&1));
/// v.pop_front();
/// assert_eq!(v.get(1), None);
///
/// // Indices for `StableIndexDeque` are stable:
/// # use re_video::StableIndexDeque;
/// let mut v = (0..2).collect::<StableIndexDeque<i32>>();
/// assert_eq!(v.get(1), Some(&1));
/// v.pop_front();
/// assert_eq!(v.get(1), Some(&1));
/// ```
#[derive(Default, Clone, Debug)]
pub struct StableIndexDeque<T> {
    vec: VecDeque<T>,
    index_offset: usize,
}

impl<T> re_byte_size::SizeBytes for StableIndexDeque<T>
where
    T: re_byte_size::SizeBytes,
{
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            vec,
            index_offset: _,
        } = self;
        vec.heap_size_bytes()
    }
}

impl<T> StableIndexDeque<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            vec: VecDeque::new(),
            index_offset: 0,
        }
    }

    /// Creates a new deque from an iterator and an index offset.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let v = StableIndexDeque::from_iter_with_offset(1, 0..2);
    /// assert_eq!(v.get(0), None);
    /// assert_eq!(v.get(1), Some(&0));
    /// assert_eq!(v.get(2), Some(&1));
    /// ```
    pub fn from_iter_with_offset(index_offset: usize, iter: impl IntoIterator<Item = T>) -> Self {
        Self {
            vec: VecDeque::from_iter(iter),
            index_offset,
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vec: VecDeque::with_capacity(capacity),
            index_offset: 0,
        }
    }

    /// See [`VecDeque::push_back`].
    #[inline]
    pub fn push_back(&mut self, value: T) {
        self.vec.push_back(value);
    }

    /// Unlike with [`VecDeque::pop_front`], indices into the deque stay the same.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..2).collect::<StableIndexDeque<i32>>();
    /// assert_eq!(v.get(0), Some(&0));
    /// assert_eq!(v.get(1), Some(&1));
    /// v.pop_front();
    /// assert_eq!(v.get(0), None);
    /// assert_eq!(v.get(1), Some(&1));
    /// ```
    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        self.vec.pop_front().inspect(|_| self.index_offset += 1)
    }

    /// See [`VecDeque::pop_back`].
    pub fn pop_back(&mut self) -> Option<T> {
        self.vec.pop_back()
    }

    /// See [`VecDeque::extend`].
    #[inline]
    pub fn extend(&mut self, values: impl IntoIterator<Item = T>) {
        self.vec.extend(values);
    }

    /// See [`VecDeque::iter`].
    ///
    /// Beware of using `.iter().enumerate()` as it will not respect the index offset.
    /// Use [`Self::iter_indexed`] instead.
    #[inline]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> {
        self.vec.iter()
    }

    /// See [`VecDeque::iter_mut`].
    ///
    /// Beware of using `.iter().enumerate()` as it will not respect the index offset.
    /// Use [`Self::iter_indexed`] instead.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.vec.iter_mut()
    }

    /// Like `iter().enumerate()` but with the index offset applied.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..2).collect::<StableIndexDeque<i32>>();
    /// v.pop_front();
    /// assert_eq!(v.iter_indexed().collect::<Vec<_>>(), vec![(1, &1)]);
    /// ```
    pub fn iter_indexed(&self) -> impl DoubleEndedIterator<Item = (usize, &T)> + ExactSizeIterator {
        let offset = self.index_offset;
        self.vec
            .iter()
            .enumerate()
            .map(move |(i, v)| (i + offset, v))
    }

    /// Like `iter_mut().enumerate()` but with the index offset applied.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..2).collect::<StableIndexDeque<i32>>();
    /// v.pop_front();
    /// assert_eq!(v.iter_indexed_mut().collect::<Vec<_>>(), vec![(1usize, &mut 1i32)]);
    /// ```
    pub fn iter_indexed_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (usize, &mut T)> + ExactSizeIterator {
        self.vec
            .iter_mut()
            .enumerate()
            .map(|(i, v)| (i + self.index_offset, v))
    }

    /// See [`VecDeque::back`].
    #[inline]
    pub fn back(&self) -> Option<&T> {
        self.vec.back()
    }

    /// See [`VecDeque::back_mut`].
    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.vec.back_mut()
    }

    /// See [`VecDeque::front`].
    #[inline]
    pub fn front(&self) -> Option<&T> {
        self.vec.front()
    }

    /// See [`VecDeque::front_mut`].
    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.vec.front_mut()
    }

    /// Removes all elements with an index larger than or equal to the given index.
    ///
    /// Truncates to the deque starting to only contain data prior (!) to the given index.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..4).collect::<StableIndexDeque<i32>>();
    /// v.pop_front();
    /// v.remove_all_with_index_larger_equal(2);
    /// assert_eq!(v.num_elements(), 1);
    /// assert_eq!(v.get(0), None);
    /// assert_eq!(v.get(1), Some(&1));
    /// assert_eq!(v.get(2), None);
    /// assert_eq!(v.get(3), None);
    /// ```
    pub fn remove_all_with_index_larger_equal(&mut self, first_index_not_contained: usize) {
        let new_len = first_index_not_contained.saturating_sub(self.index_offset);
        self.vec.truncate(new_len);
    }

    /// Removes all elements with an index smaller or equal than the given index.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..4).collect::<StableIndexDeque<i32>>();
    /// v.remove_all_with_index_smaller_equal(1);
    /// assert_eq!(v.get(0), None);
    /// assert_eq!(v.get(1), None);
    /// assert_eq!(v.get(2), Some(&2));
    /// assert_eq!(v.get(3), Some(&3));
    /// ```
    pub fn remove_all_with_index_smaller_equal(&mut self, first_index_contained: usize) {
        while !self.vec.is_empty() && first_index_contained >= self.index_offset {
            self.pop_front();
        }
    }

    /// [`Iterator::position`] but with the index offset applied.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..4).collect::<StableIndexDeque<i32>>();
    /// v.pop_front();
    /// assert_eq!(v.position(|&x| x == 2), Some(2));
    /// v.pop_front();
    /// assert_eq!(v.position(|&x| x == 2), Some(2));
    /// ```
    pub fn position(&self, predicate: impl Fn(&T) -> bool) -> Option<usize> {
        self.vec
            .iter()
            .position(predicate)
            .map(|i| i + self.index_offset)
    }

    /// [`VecDeque::partition_point`] but with the index offset applied.
    #[inline]
    pub fn partition_point<F>(&self, f: F) -> usize
    where
        F: FnMut(&T) -> bool,
    {
        self.vec.partition_point(f) + self.index_offset
    }

    /// Whether there is no data in this deque.
    ///
    /// The internal offset may still be non-zero!
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    /// Retrieves an element by index.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.vec.get(index.checked_sub(self.index_offset)?)
    }

    /// Retrieves a mutable element by index.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.vec.get_mut(index.checked_sub(self.index_offset)?)
    }

    /// The next index that will be used if we push a new element.
    ///
    /// Note that we do not expose `len` to avoid confusion.
    /// See also [`Self::num_elements`].
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..2).collect::<StableIndexDeque<i32>>();
    /// assert_eq!(v.next_index(), 2);
    /// v.pop_front();
    /// assert_eq!(v.next_index(), 2);
    /// ```
    #[inline]
    pub fn next_index(&self) -> usize {
        self.vec.len() + self.index_offset
    }

    /// The smallest index that is still valid for accessing elements in this deque if its non-empty.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..2).collect::<StableIndexDeque<i32>>();
    /// assert_eq!(v.min_index(), 0);
    /// v.pop_front();
    /// assert_eq!(v.min_index(), 1);
    /// v.pop_front();
    /// assert_eq!(v.min_index(), 2);
    /// ```
    #[inline]
    pub fn min_index(&self) -> usize {
        self.index_offset
    }

    /// The number of elements currently stored in this deque.
    ///
    /// Ignores the internal offset.
    /// Note that we do not expose `len` to avoid confusion.
    /// See also [`Self::next_index`].
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..1).collect::<StableIndexDeque<i32>>();
    /// assert_eq!(v.num_elements(), 1);
    /// v.pop_front();
    /// assert_eq!(v.num_elements(), 0);
    /// ```
    #[inline]
    pub fn num_elements(&self) -> usize {
        self.vec.len()
    }

    /// Iterates over an index range which is truncated to a valid range in the list.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..5).collect::<StableIndexDeque<i32>>();
    /// v.pop_front();
    /// assert_eq!(v.iter_index_range_clamped(&(0..5)).collect::<Vec<_>>(), vec![(1, &1), (2, &2), (3, &3), (4, &4)]);
    /// assert_eq!(v.iter_index_range_clamped(&(2..4)).collect::<Vec<_>>(), vec![(2, &2), (3, &3)]);
    /// assert_eq!(v.iter_index_range_clamped(&(3..5)).collect::<Vec<_>>(), vec![(3, &3), (4, &4)]);
    /// ```
    #[inline]
    pub fn iter_index_range_clamped<'a>(
        &'a self,
        range: &std::ops::Range<usize>,
    ) -> impl DoubleEndedIterator<Item = (usize, &'a T)> + ExactSizeIterator + use<'a, T> {
        let range_start = range.start.saturating_sub(self.index_offset);
        let num_elements = range.end - range.start;
        self.iter_indexed().skip(range_start).take(num_elements)
    }

    /// Mutably iterates over an index range which is truncated to a valid range in
    /// the list.
    ///
    /// ```
    /// # use re_video::StableIndexDeque;
    /// let mut v = (0..5).collect::<StableIndexDeque<i32>>();
    /// v.pop_front();
    /// assert_eq!(v.iter_index_range_clamped_mut(&(0..5)).collect::<Vec<_>>(), vec![(1, &mut 1), (2, &mut 2), (3, &mut 3), (4, &mut 4)]);
    /// assert_eq!(v.iter_index_range_clamped_mut(&(2..4)).collect::<Vec<_>>(), vec![(2, &mut 2), (3, &mut 3)]);
    /// assert_eq!(v.iter_index_range_clamped_mut(&(3..5)).collect::<Vec<_>>(), vec![(3, &mut 3), (4, &mut 4)]);
    /// ```
    #[inline]
    pub fn iter_index_range_clamped_mut<'a>(
        &'a mut self,
        range: &std::ops::Range<usize>,
    ) -> impl DoubleEndedIterator<Item = (usize, &'a mut T)> + ExactSizeIterator + use<'a, T> {
        let range_start = range.start.saturating_sub(self.index_offset);
        let num_elements = range.end - range.start;
        self.iter_indexed_mut().skip(range_start).take(num_elements)
    }
}

impl<T> std::ops::Index<usize> for StableIndexDeque<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.vec[index - self.index_offset]
    }
}

impl<T> std::ops::IndexMut<usize> for StableIndexDeque<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.vec[index - self.index_offset]
    }
}

impl<T> FromIterator<T> for StableIndexDeque<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            vec: VecDeque::from_iter(iter),
            index_offset: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StableIndexDeque;

    #[test]
    fn test_stable_index_deque() {
        let mut vec = StableIndexDeque::new();
        vec.push_back(1);
        vec.push_back(2);
        assert_eq!(vec[0], 1);
        assert_eq!(vec[1], 2);
        assert_eq!(vec.next_index(), 2);
        assert_eq!(vec.num_elements(), 2);
        assert_eq!(vec.min_index(), 0);

        vec.pop_front();
        assert_eq!(vec.get(0), None);
        assert_eq!(vec.get(1), Some(&2));
        assert_eq!(vec[1], 2);
        assert_eq!(vec.get(2), None);
        assert_eq!(vec.next_index(), 2);
        assert_eq!(vec.vec.len(), 1);
        assert_eq!(vec.num_elements(), 1);
        assert_eq!(vec.min_index(), 1);

        vec.pop_front();
        assert_eq!(vec.vec.len(), 0);
        assert_eq!(vec.next_index(), 2);
        assert_eq!(vec.num_elements(), 0);
        assert_eq!(vec.min_index(), 2);
    }
}
