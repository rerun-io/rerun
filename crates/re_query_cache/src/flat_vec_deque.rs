use std::{collections::VecDeque, ops::Range};

use itertools::Itertools;

use re_types_core::SizeBytes;

// ---

/// A [`FlatVecDeque`] that can be erased into a trait object.
///
/// Methods that don't require monomorphization over `T` are made dynamically dispatchable.
pub trait ErasedFlatVecDeque: std::any::Any {
    fn as_any(&self) -> &dyn std::any::Any;

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;

    /// Dynamically dispatches to [`FlatVecDeque::num_entries`].
    ///
    /// This is prefixed with `dyn_` to avoid method dispatch ambiguities that are very hard to
    /// avoid even with explicit syntax and that silently lead to infinite recursions.
    fn dyn_num_entries(&self) -> usize;

    /// Dynamically dispatches to [`FlatVecDeque::num_values`].
    ///
    /// This is prefixed with `dyn_` to avoid method dispatch ambiguities that are very hard to
    /// avoid even with explicit syntax and that silently lead to infinite recursions.
    fn dyn_num_values(&self) -> usize;

    /// Dynamically dispatches to [`FlatVecDeque::remove`].
    ///
    /// This is prefixed with `dyn_` to avoid method dispatch ambiguities that are very hard to
    /// avoid even with explicit syntax and that silently lead to infinite recursions.
    fn dyn_remove(&mut self, at: usize);

    /// Dynamically dispatches to [`FlatVecDeque::remove`].
    ///
    /// This is prefixed with `dyn_` to avoid method dispatch ambiguities that are very hard to
    /// avoid even with explicit syntax and that silently lead to infinite recursions.
    fn dyn_remove_range(&mut self, range: Range<usize>);

    /// Dynamically dispatches to [`FlatVecDeque::truncate`].
    ///
    /// This is prefixed with `dyn_` to avoid method dispatch ambiguities that are very hard to
    /// avoid even with explicit syntax and that silently lead to infinite recursions.
    fn dyn_truncate(&mut self, at: usize);

    /// Dynamically dispatches to [`<FlatVecDeque<T> as SizeBytes>::total_size_bytes(self)`].
    ///
    /// This is prefixed with `dyn_` to avoid method dispatch ambiguities that are very hard to
    /// avoid even with explicit syntax and that silently lead to infinite recursions.
    fn dyn_total_size_bytes(&self) -> u64;
}

impl<T: SizeBytes + 'static> ErasedFlatVecDeque for FlatVecDeque<T> {
    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    #[inline]
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    #[inline]
    fn dyn_num_entries(&self) -> usize {
        self.num_entries()
    }

    #[inline]
    fn dyn_num_values(&self) -> usize {
        self.num_values()
    }

    #[inline]
    fn dyn_remove(&mut self, at: usize) {
        FlatVecDeque::<T>::remove(self, at);
    }

    #[inline]
    fn dyn_remove_range(&mut self, range: Range<usize>) {
        FlatVecDeque::<T>::remove_range(self, range);
    }

    #[inline]
    fn dyn_truncate(&mut self, at: usize) {
        FlatVecDeque::<T>::truncate(self, at);
    }

    #[inline]
    fn dyn_total_size_bytes(&self) -> u64 {
        <FlatVecDeque<T> as SizeBytes>::total_size_bytes(self)
    }
}

// ---

/// A double-ended queue implemented with a pair of growable ring buffers, where every single
/// entry is a flattened array of values.
///
/// Logically like a `VecDeque<Box<[T]>>`, but with a less fragmented memory layout (each `Box<[T]>`
/// gets copied/inlined into the `FlatVecDeque`).
/// `FlatVecDeque` therefore optimizes for reads (cache locality, specifically) while `VecDeque<Box<[T]>>`
/// optimizes for writes.
///
/// You can think of this as the native/deserialized version of an Arrow `ListArray`.
/// This is particularly useful when working with many small arrays of data (e.g. Rerun's
/// `TimeSeriesScalar`s).
//
// TODO(cmc): We could even use a bitmap for T=Option<Something>, which would bring this that much
// closer to a deserialized version of an Arrow array.
#[derive(Debug, Clone)]
pub struct FlatVecDeque<T> {
    /// Stores every value in the `FlatVecDeque` in a flattened `VecDeque`.
    ///
    /// E.g.:
    /// - `FlatVecDeque[]` -> values=`[]`.
    /// - `FlatVecDeque[[], [], []]` -> values=`[]`.
    /// - `FlatVecDeque[[], [0], [1, 2, 3], [4, 5]]` -> values=`[0, 1, 2, 3, 4, 5]`.
    values: VecDeque<T>,

    /// Keeps track of each entry, i.e. logical slices of data.
    ///
    /// E.g.:
    /// - `FlatVecDeque[]` -> offsets=`[]`.
    /// - `FlatVecDeque[[], [], []]` -> offsets=`[0, 0, 0]`.
    /// - `FlatVecDeque[[], [0], [1, 2, 3], [4, 5]]` -> offsets=`[0, 1, 4, 6]`.
    offsets: VecDeque<usize>,
}

impl<T: SizeBytes> SizeBytes for FlatVecDeque<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.

        let values_size_bytes = if T::is_pod() {
            (self.num_values() * std::mem::size_of::<T>()) as _
        } else {
            self.values
                .iter()
                .map(SizeBytes::total_size_bytes)
                .sum::<u64>()
        };

        let offsets_size_bytes = self.num_entries() * std::mem::size_of::<usize>();

        values_size_bytes + offsets_size_bytes as u64
    }
}

impl<T> From<VecDeque<T>> for FlatVecDeque<T> {
    #[inline]
    fn from(values: VecDeque<T>) -> Self {
        let num_values = values.len();
        Self {
            values,
            offsets: std::iter::once(num_values).collect(),
        }
    }
}

impl<T> Default for FlatVecDeque<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FlatVecDeque<T> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            values: VecDeque::new(),
            offsets: VecDeque::new(),
        }
    }

    #[inline]
    pub fn from_vecs(entries: impl IntoIterator<Item = Vec<T>>) -> Self {
        let mut this = Self::new();

        // NOTE: Do not use any of the insertion methods, they rely on `from_vecs` in the first
        // place!
        let mut value_offset = 0;
        for entry in entries {
            value_offset += entry.len(); // increment first!
            this.offsets.push_back(value_offset);
            this.values.extend(entry);
        }

        this
    }

    /// How many entries are there in the deque?
    ///
    /// Keep in mind: each entry is itself an array of values.
    /// Use [`Self::num_values`] to get the total number of values across all entries.
    #[inline]
    pub fn num_entries(&self) -> usize {
        self.offsets.len()
    }

    /// How many values are there in the deque?
    ///
    /// Keep in mind: each entry in the deque holds an array of values.
    /// Use [`Self::num_entries`] to get the total number of entries, irrelevant of how many
    /// values each entry holds.
    #[inline]
    pub fn num_values(&self) -> usize {
        self.values.len()
    }

    #[inline]
    fn value_offset(&self, entry_index: usize) -> usize {
        if entry_index == 0 {
            0
        } else {
            self.offsets[entry_index - 1]
        }
    }

    #[inline]
    fn iter_offset_ranges(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        std::iter::once(0)
            .chain(self.offsets.iter().copied())
            .tuple_windows::<(_, _)>()
            .map(|(start, end)| (start..end))
    }
}

// ---

impl<T> FlatVecDeque<T> {
    /// Iterates over all the entries in the deque.
    ///
    /// This is the same as `self.range(0..self.num_entries())`.
    ///
    /// Keep in mind that each entry is an array of values!
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &[T]> {
        self.range(0..self.num_entries())
    }

    /// Iterates over all the entries in the deque in the given `entry_range`.
    ///
    /// Keep in mind that each entry is an array of values!
    // #[inline]
    pub fn range(&self, entry_range: Range<usize>) -> impl Iterator<Item = &[T]> {
        eprintln!("{:?}", self.offsets);
        let (values_left, values_right) = self.values.as_slices();
        // dbg!((
        //     &entry_range,
        //     entry_range.len(),
        //     // &self.offsets,
        //     self.offsets.len(),
        //     self.values.len(),
        //     values_left.len(),
        //     values_right.len(),
        //     self.iter_offset_ranges().collect_vec(),
        // ));
        // NOTE: We can't slice into our offsets, we don't even know if they're contiguous in
        // memory at this point -> skip() and take().
        self.iter_offset_ranges()
            .skip(entry_range.start)
            .take(entry_range.len())
            .map(|offsets| {
                if offsets.is_empty() {
                    return &[] as &'_ [T];
                }
                // dbg!((
                //     &offsets,
                //     offsets.start,
                //     values_left.len(),
                //     values_right.len()
                // ));
                // NOTE: We do not need `make_contiguous` here because we always guarantee
                // that a single entry's worth of values is fully contained in either the left or
                // right buffer, but never straddling across both.
                if offsets.start < values_left.len() {
                    &values_left[offsets]
                } else {
                    &values_right[offsets]
                }
            })
    }
}

#[test]
fn range() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert_many(0, [vec![1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10]]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &v);

    assert_iter_eq(&[&[1, 2, 3]], v.range(0..1));
    assert_iter_eq(&[&[4, 5, 6, 7]], v.range(1..2));
    assert_iter_eq(&[&[8, 9, 10]], v.range(2..3));

    assert_iter_eq(
        &[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]],
        v.range(0..v.num_entries()),
    );

    assert_iter_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], v.iter());
}

// ---

impl<T> FlatVecDeque<T> {
    /// Prepends an entry comprised of `values` to the deque.
    ///
    /// This is the same as `self.insert(0, values)`.
    ///
    /// See [`Self::insert`] for more information.
    #[inline]
    pub fn push_front(&mut self, values: impl IntoIterator<Item = T>) {
        self.insert(0, values);
    }

    /// Appends an entry comprised of `values` to the deque.
    ///
    /// This is the same as `self.insert(self.num_entries(), values)`.
    ///
    /// See [`Self::insert`] for more information.
    #[inline]
    pub fn push_back(&mut self, values: impl IntoIterator<Item = T>) {
        self.insert(self.num_entries(), values);
    }

    /// Inserts a single entry at `entry_index`, comprised of the multiple elements given as `values`.
    ///
    /// This is O(1) if `entry_index` corresponds to either the start or the end of the deque.
    /// Otherwise, this requires splitting the deque into two pieces then stitching them back together
    /// at both ends of the added data.
    ///
    /// Panics if `entry_index` is out of bounds.
    /// Panics if `values` is empty.
    #[inline]
    pub fn insert(&mut self, entry_index: usize, values: impl IntoIterator<Item = T>) {
        let values: VecDeque<T> = values.into_iter().collect();
        let deque = values.into();
        self.insert_deque(entry_index, deque);
    }

    /// Prepends multiple entries, each comprised of the multiple elements given in `entries`,
    /// to the deque.
    ///
    /// This is the same as `self.insert_many(0, entries)`.
    ///
    /// See [`Self::insert_many`] for more information.
    #[inline]
    pub fn push_many_front(&mut self, entries: impl IntoIterator<Item = Vec<T>>) {
        self.insert_many(0, entries);
    }

    /// Appends multiple entries, each comprised of the multiple elements given in `entries`,
    /// to the deque.
    ///
    /// This is the same as `self.insert_many(self.num_entries(), entries)`.
    ///
    /// See [`Self::insert_many`] for more information.
    #[inline]
    pub fn push_many_back(&mut self, entries: impl IntoIterator<Item = Vec<T>>) {
        self.insert_many(self.num_entries(), entries);
    }

    /// Inserts multiple entries, starting at `entry_index` onwards, each comprised of the multiple elements
    /// given in `entries`.
    ///
    /// This is O(1) if `entry_index` corresponds to either the start or the end of the deque.
    /// Otherwise, this requires splitting the deque into two pieces then stitching them back together
    /// at both ends of the added data.
    ///
    /// Panics if `entry_index` is out of bounds.
    /// Panics if any of the value arrays in `entries` is empty.
    #[inline]
    pub fn insert_many(&mut self, entry_index: usize, entries: impl IntoIterator<Item = Vec<T>>) {
        let deque = Self::from_vecs(entries);
        self.insert_deque(entry_index, deque);
    }

    /// Prepends another full deque to the deque.
    ///
    /// This is the same as `self.insert_deque(0, rhs)`.
    ///
    /// See [`Self::insert_deque`] for more information.
    #[inline]
    pub fn push_front_deque(&mut self, rhs: FlatVecDeque<T>) {
        self.insert_deque(0, rhs);
    }

    /// Appends another full deque to the deque.
    ///
    /// This is the same as `self.insert_deque(0, rhs)`.
    ///
    /// See [`Self::insert_deque`] for more information.
    #[inline]
    pub fn push_back_deque(&mut self, rhs: FlatVecDeque<T>) {
        self.insert_deque(self.num_entries(), rhs);
    }

    /// Inserts another full deque, starting at `entry_index` and onwards.
    ///
    /// This is O(1) if `entry_index` corresponds to either the start or the end of the deque.
    /// Otherwise, this requires splitting the deque into two pieces then stitching them back together
    /// at both ends of the added data.
    ///
    /// Panics if `entry_index` is out of bounds.
    /// Panics if any of the value arrays in `entries` is empty.
    pub fn insert_deque(&mut self, entry_index: usize, mut rhs: FlatVecDeque<T>) {
        // NOTE: We're inserting _beyond_ the last element.
        if entry_index == self.num_entries() {
            let max_value_offset = self.offsets.back().copied().unwrap_or_default();
            self.offsets
                .extend(rhs.offsets.into_iter().map(|o| o + max_value_offset));
            self.values.extend(rhs.values);
            return;
        } else if entry_index == 0 {
            rhs.push_back_deque(std::mem::take(self));
            *self = rhs;
            return;
        }

        let right = self.split_off(entry_index);
        self.push_back_deque(rhs);
        self.push_back_deque(right);

        debug_assert!(self.iter_offset_ranges().all(|r| r.start < r.end));
    }
}

#[test]
fn insert() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    // TODO: dont commit this
    fn assert_offsets_eq(expected: &[usize], got: &VecDeque<usize>) {
        similar_asserts::assert_eq!(expected, got.clone().into_iter().collect_vec());
    }

    v.insert(0, [1, 2, 3]);
    assert_offsets_eq(&[3], &v.offsets);
    assert_deque_eq(&[&[1, 2, 3]], &v);

    v.insert(0, [4, 5, 6, 7]);
    assert_offsets_eq(&[4, 7], &v.offsets);
    assert_deque_eq(&[&[4, 5, 6, 7], &[1, 2, 3]], &v);

    v.insert(0, [8, 9]);
    assert_offsets_eq(&[2, 6, 9], &v.offsets);
    assert_deque_eq(&[&[8, 9], &[4, 5, 6, 7], &[1, 2, 3]], &v);

    v.insert(2, [10, 11, 12, 13]);
    assert_offsets_eq(&[2, 6, 10, 13], &v.offsets);
    assert_deque_eq(&[&[8, 9], &[4, 5, 6, 7], &[10, 11, 12, 13], &[1, 2, 3]], &v);

    v.insert(v.num_entries(), [14, 15]);
    assert_offsets_eq(&[2, 6, 10, 13, 15], &v.offsets);
    assert_deque_eq(
        &[
            &[8, 9],
            &[4, 5, 6, 7],
            &[10, 11, 12, 13],
            &[1, 2, 3],
            &[14, 15],
        ],
        &v,
    );

    v.insert(v.num_entries() - 1, [42]);
    assert_offsets_eq(&[2, 6, 10, 13, 14, 16], &v.offsets);
    assert_deque_eq(
        &[
            &[8, 9],
            &[4, 5, 6, 7],
            &[10, 11, 12, 13],
            &[1, 2, 3],
            &[42],
            &[14, 15],
        ],
        &v,
    );
}

#[test]
fn insert_empty() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.push_back([]);
    v.push_back([]);
    v.push_back([]);

    assert_deque_eq(&[&[], &[], &[]], &v);
}

// Simulate the bug that was making everything crash on the face tracking example.
#[test]
fn insert_some_and_empty() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.push_back([0]);

    eprintln!("{:?}", v.offsets);
    assert_deque_eq(&[&[0]], &v);

    v.push_back([]);

    eprintln!("{:?}", v.offsets);
    assert_deque_eq(&[&[0], &[]], &v);

    return;

    v.push_back([1]);
    v.push_back([]);

    v.push_back([2]);
    v.push_back([]);

    assert_deque_eq(&[&[0], &[], &[1], &[], &[2], &[]], &v);
}

#[test]
fn insert_many() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert_many(0, [vec![1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10]]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &v);

    v.insert_many(0, [vec![20], vec![21], vec![22]]);
    assert_deque_eq(
        &[&[20], &[21], &[22], &[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]],
        &v,
    );

    v.insert_many(4, [vec![41, 42], vec![43]]);
    assert_deque_eq(
        &[
            &[20],
            &[21],
            &[22],
            &[1, 2, 3],
            &[41, 42],
            &[43],
            &[4, 5, 6, 7],
            &[8, 9, 10],
        ],
        &v,
    );

    v.insert_many(v.num_entries(), [vec![100], vec![200, 300, 400]]);
    assert_deque_eq(
        &[
            &[20],
            &[21],
            &[22],
            &[1, 2, 3],
            &[41, 42],
            &[43],
            &[4, 5, 6, 7],
            &[8, 9, 10],
            &[100],
            &[200, 300, 400],
        ],
        &v,
    );
}

#[test]
fn insert_deque() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert_deque(
        0,
        FlatVecDeque::from_vecs([vec![1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10]]),
    );
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &v);

    v.insert_deque(0, FlatVecDeque::from_vecs([vec![20], vec![21], vec![22]]));
    assert_deque_eq(
        &[&[20], &[21], &[22], &[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]],
        &v,
    );

    v.insert_deque(4, FlatVecDeque::from_vecs([vec![41, 42], vec![43]]));
    assert_deque_eq(
        &[
            &[20],
            &[21],
            &[22],
            &[1, 2, 3],
            &[41, 42],
            &[43],
            &[4, 5, 6, 7],
            &[8, 9, 10],
        ],
        &v,
    );

    v.insert_deque(
        v.num_entries(),
        FlatVecDeque::from_vecs([vec![100], vec![200, 300, 400]]),
    );
    assert_deque_eq(
        &[
            &[20],
            &[21],
            &[22],
            &[1, 2, 3],
            &[41, 42],
            &[43],
            &[4, 5, 6, 7],
            &[8, 9, 10],
            &[100],
            &[200, 300, 400],
        ],
        &v,
    );
}

// ---

impl<T> FlatVecDeque<T> {
    /// Splits the deque into two at the given index.
    ///
    /// Returns a newly allocated `FlatVecDeque`. `self` contains entries `[0, entry_index)`,
    /// and the returned deque contains entries `[entry_index, num_entries)`.
    ///
    /// Note that the capacity of `self` does not change.
    ///
    /// Panics if `entry_index` is out of bounds.
    #[inline]
    #[must_use = "use `.truncate()` if you don't need the other half"]
    pub fn split_off(&mut self, entry_index: usize) -> Self {
        let value_offset = self.value_offset(entry_index);

        let mut offsets = self.offsets.split_off(entry_index);
        for offset in &mut offsets {
            *offset -= value_offset;
        }

        Self {
            values: self.values.split_off(value_offset),
            offsets,
        }
    }

    /// Shortens the deque, keeping all entries up to `entry_index` (excluded), and
    /// dropping the rest.
    ///
    /// Panics if `entry_index` is out of bounds.
    #[inline]
    pub fn truncate(&mut self, entry_index: usize) {
        self.values.truncate(self.value_offset(entry_index));
        self.offsets.truncate(entry_index);
    }

    /// Removes the entry at `entry_index` from the deque.
    ///
    /// This is O(1) if `entry_index` corresponds to either the start or the end of the deque.
    /// Otherwise, this requires splitting the deque into three pieces, dropping the superfluous
    /// one, then stitching the two remaining pices back together.
    ///
    /// Panics if `entry_index` is out of bounds.
    pub fn remove(&mut self, entry_index: usize) {
        let (start_offset, end_offset) = (
            self.value_offset(entry_index),
            self.value_offset(entry_index + 1),
        );
        let offset_count = end_offset - start_offset;

        if entry_index + 1 == self.num_entries() {
            self.offsets.truncate(self.num_entries() - 1);
            self.values.truncate(self.values.len() - offset_count);
            return;
        } else if entry_index == 0 {
            *self = self.split_off(entry_index + 1);
            return;
        }

        // NOTE: elegant, but way too slow :)
        // let right = self.split_off(entry_index + 1);
        // _ = self.split_off(self.num_entries() - 1);
        // self.push_back_deque(right);

        _ = self.offsets.remove(entry_index);
        for offset in self.offsets.range_mut(entry_index..) {
            *offset -= offset_count;
        }

        let right = self.values.split_off(end_offset);
        self.values.truncate(self.values.len() - offset_count);
        self.values.extend(right);
    }

    /// Removes all entries within the given `entry_range` from the deque.
    ///
    /// This is O(1) if `entry_range` either starts at the beginning of the deque, or ends at
    /// the end of the deque, or both.
    /// Otherwise, this requires splitting the deque into three pieces, dropping the superfluous
    /// one, then stitching the two remaining pieces back together.
    ///
    /// Panics if `entry_range` is either out of bounds or isn't monotonically increasing.
    #[inline]
    pub fn remove_range(&mut self, entry_range: Range<usize>) {
        assert!(entry_range.start <= entry_range.end);

        if entry_range.start == entry_range.end {
            return;
        }

        let (start_offset, end_offset) = (
            self.value_offset(entry_range.start),
            self.value_offset(entry_range.end),
        );
        let offset_count = end_offset - start_offset;

        // Reminder: `entry_range.end` is exclusive.
        if entry_range.end == self.num_entries() {
            self.offsets
                .truncate(self.num_entries() - entry_range.len());
            self.values.truncate(self.values.len() - offset_count);
            return;
        } else if entry_range.start == 0 {
            *self = self.split_off(entry_range.end);
            return;
        }

        let right = self.split_off(entry_range.end);
        _ = self.split_off(self.num_entries() - entry_range.len());
        self.push_back_deque(right);
    }
}

#[test]
fn truncate() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert_many(0, [vec![1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10]]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &v);

    {
        let mut v = v.clone();
        v.truncate(0);
        assert_deque_eq(&[], &v);
    }

    {
        let mut v = v.clone();
        v.truncate(1);
        assert_deque_eq(&[&[1, 2, 3]], &v);
    }

    {
        let mut v = v.clone();
        v.truncate(2);
        assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7]], &v);
    }

    {
        let mut v = v.clone();
        v.truncate(3);
        assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &v);
    }
}

#[test]
fn split_off() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert_many(0, [vec![1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10]]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &v);

    {
        let mut left = v.clone();
        let right = left.split_off(0);

        assert_deque_eq(&[], &left);
        assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &right);
    }

    {
        let mut left = v.clone();
        let right = left.split_off(1);

        assert_deque_eq(&[&[1, 2, 3]], &left);
        assert_deque_eq(&[&[4, 5, 6, 7], &[8, 9, 10]], &right);
    }

    {
        let mut left = v.clone();
        let right = left.split_off(2);

        assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7]], &left);
        assert_deque_eq(&[&[8, 9, 10]], &right);
    }

    {
        let mut left = v.clone();
        let right = left.split_off(3);

        assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &left);
        assert_deque_eq(&[], &right);
    }
}

#[test]
fn remove() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert(0, [1, 2, 3]);
    assert_deque_eq(&[&[1, 2, 3]], &v);

    v.remove(0);
    assert_deque_eq(&[], &v);

    v.insert(0, [1, 2, 3]);
    assert_deque_eq(&[&[1, 2, 3]], &v);

    v.insert(1, [4, 5, 6, 7]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7]], &v);

    v.insert(2, [8, 9]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9]], &v);

    v.remove(0);
    assert_deque_eq(&[&[4, 5, 6, 7], &[8, 9]], &v);

    v.insert(0, [1, 2, 3]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9]], &v);

    v.remove(1);
    assert_deque_eq(&[&[1, 2, 3], &[8, 9]], &v);

    v.insert(1, [4, 5, 6, 7]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9]], &v);

    v.remove(2);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7]], &v);

    v.remove(0);
    assert_deque_eq(&[&[4, 5, 6, 7]], &v);

    v.remove(0);
    assert_deque_eq(&[], &v);
}

#[test]
#[should_panic(expected = "Out of bounds access")]
fn remove_empty() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.remove(0);
}

#[test]
#[should_panic(expected = "Out of bounds access")]
fn remove_oob() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert(0, [1, 2, 3]);
    assert_deque_eq(&[&[1, 2, 3]], &v);

    assert_eq!(1, v.num_entries());
    assert_eq!(3, v.num_values());

    v.remove(1);
}

#[test]
fn remove_range() {
    let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

    assert_eq!(0, v.num_entries());
    assert_eq!(0, v.num_values());

    v.insert_many(0, [vec![1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10]]);
    assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7], &[8, 9, 10]], &v);

    {
        let mut v = v.clone();
        v.remove_range(0..1);
        assert_deque_eq(&[&[4, 5, 6, 7], &[8, 9, 10]], &v);
    }

    {
        let mut v = v.clone();
        v.remove_range(1..2);
        assert_deque_eq(&[&[1, 2, 3], &[8, 9, 10]], &v);
    }

    {
        let mut v = v.clone();
        v.remove_range(2..3);
        assert_deque_eq(&[&[1, 2, 3], &[4, 5, 6, 7]], &v);
    }

    {
        let mut v = v.clone();
        v.remove_range(0..2);
        assert_deque_eq(&[&[8, 9, 10]], &v);
    }

    {
        let mut v = v.clone();
        v.remove_range(1..3);
        assert_deque_eq(&[&[1, 2, 3]], &v);
    }

    {
        let mut v = v.clone();
        v.remove_range(0..3);
        assert_deque_eq(&[], &v);
    }
}

// ---

#[cfg(test)]
fn assert_deque_eq(expected: &[&'_ [i64]], got: &FlatVecDeque<i64>) {
    similar_asserts::assert_eq!(expected, got.iter().collect_vec());
}

#[cfg(test)]
fn assert_iter_eq<'a>(expected: &[&'_ [i64]], got: impl Iterator<Item = &'a [i64]>) {
    similar_asserts::assert_eq!(expected, got.collect_vec());
}
