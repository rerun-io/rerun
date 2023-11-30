use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
};

use itertools::Itertools;

use re_types_core::SizeBytes;

// ---

// TODO: the assumption is that the indices are full...?

// TODO
const MAX_BUCKET_ENTRIES: usize = 1_000;

// TODO: better name?
// TODO: should probably do nullability with a bitmap too then...
// TODO: that this looks and feels like arrow is no coincidence, we'll probably want to just re-use
// the arrow storage directly where we can (i.e. no transformation)!
// TODO: i guess ideally we shouldn't even take space for Nones... keeping a bitmap would allow us
// to iterate as we need... we even already depend on arrow
#[derive(Debug, Clone)]
pub struct FlatVecDeque<T> {
    data: VecDeque<T>,
    offsets: VecDeque<usize>,
}

impl<T: SizeBytes> SizeBytes for FlatVecDeque<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // TODO: pod optimization
        self.iter().map(SizeBytes::total_size_bytes).sum::<u64>()
    }
}

impl<T> Default for FlatVecDeque<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FlatVecDeque<T> {
    pub const fn new() -> Self {
        Self {
            data: VecDeque::new(),
            offsets: VecDeque::new(),
        }
    }

    // TODO: document entries vs len

    pub fn num_entries(&self) -> usize {
        self.offsets.len()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.num_entries() == 0
    }

    pub fn extend_back(&mut self, values: impl IntoIterator<Item = T>) {
        re_tracing::profile_function!();

        self.data.extend(values);
        // TODO: figure out how costly => should be free if all inserts are in order, right?
        self.data.make_contiguous();

        self.offsets.push_back(self.data.len());
        self.offsets.make_contiguous();

        // self.last_bucket_mut(move |bucket| bucket.extend_back(values));
    }

    pub fn iter_offsets(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        std::iter::once(0)
            .chain(self.offsets.iter().copied())
            .tuple_windows::<(_, _)>()
            .map(|(start, end)| (start..end))
    }

    pub fn drain(
        &mut self,
        range: Range<usize>,
    ) -> (
        impl Iterator<Item = T> + '_,
        impl Iterator<Item = usize> + '_,
    ) {
        (self.data.drain(range.clone()), self.offsets.drain(range))
    }

    pub fn iter(&self) -> impl Iterator<Item = &[T]> {
        // DO _NOT_ DO THIS WITHIN THE ITERATOR ITSELF!!!
        let (slice, &[]) = self.data.as_slices() else {
            panic!("TODO");
        };

        std::iter::once(0)
            .chain(self.offsets.iter().copied())
            .tuple_windows::<(_, _)>()
            .map(|(start, end)| &slice[start..end])
    }

    pub fn range(&self, range: Range<usize>) -> impl Iterator<Item = &[T]> {
        // DO _NOT_ DO THIS WITHIN THE ITERATOR ITSELF!!!
        let (offsets, &[]) = self.offsets.as_slices() else {
            panic!("TODO");
        };
        let (slice, &[]) = self.data.as_slices() else {
            panic!("TODO");
        };

        std::iter::once(0)
            .chain(offsets[range.start..range.end].iter().copied())
            .tuple_windows::<(_, _)>()
            .map(|(start, end)| &slice[start..end])
    }

    pub fn extend_back_with(&mut self, rhs: FlatVecDeque<T>) {
        re_tracing::profile_function!();

        let max_offset = self.offsets.back().copied().unwrap_or_default();
        self.offsets
            .extend(rhs.offsets.into_iter().map(|o| o + max_offset));
        self.offsets.make_contiguous();

        self.data.extend(rhs.data);

        // TODO: figure out how costly => should be free if all inserts are in order, right?
        self.data.make_contiguous();
    }
}

impl<T: std::fmt::Debug> FlatVecDeque<T> {
    // TODO: test + bench fwd bwd middle
    pub fn remove_at(&mut self, at: usize) {
        let Some(range) = self.iter_offsets().nth(at) else {
            return;
        };

        re_tracing::profile_function!();

        let remove_front = at == 0;
        let remove_back = at.saturating_add(1) == self.offsets.len();

        let Some(_) = self.offsets.remove(at) else {
            if cfg!(debug_assertions) {
                panic!("corrupt offsets");
            }
            return;
        };
        self.offsets.make_contiguous();

        for offset in self.offsets.range_mut(at..) {
            *offset -= range.len(); // TODO: lots of unsaturated stuff
        }

        if remove_front {
            self.data.rotate_left(range.len());
            self.data.truncate(self.data.len() - range.len());
            self.data.make_contiguous();
            return;
        }

        if remove_back {
            self.data.truncate(self.data.len() - range.len());
            self.data.make_contiguous();
            return;
        }

        let right = {
            re_tracing::profile_scope!("split");
            let right = self.data.split_off(range.end);
            self.data = std::mem::take(&mut self.data);
            right
        };

        self.data.truncate(self.data.len() - range.len());
        self.data.extend(right);
        self.data.make_contiguous();
    }
}

// TODO: merge with upstairs
impl<T> FlatVecDeque<T> {
    // TODO: benchmarks would be nice:
    // - front
    // - center
    // - back
    //
    // TODO: `at` has to be a logical position in this case (i.e. an index in `offsets`)
    pub fn extend_at(&mut self, at: usize, values: impl ExactSizeIterator<Item = T>) {
        if values.len() == 0 {
            return;
        }

        let extend_back = self.offsets.len() == at;
        if extend_back {
            return self.extend_back(values);
        }

        re_tracing::profile_function!();

        let offset = match at {
            0 => {
                self.offsets.insert(at, values.len());
                0
            }
            at => {
                let offset = self.offsets[at - 1];
                self.offsets.insert(at, offset + values.len());
                offset
            }
        };

        self.offsets.make_contiguous();

        for offset in self.offsets.range_mut(at + 1..) {
            *offset += values.len();
        }

        // TODO: if we cannot make this fast then i guess we gotta bucketized the flatvecdeque
        // itself

        let right = {
            re_tracing::profile_scope!("split");
            let right = self.data.split_off(offset);
            self.data = std::mem::take(&mut self.data);
            right
        };

        {
            re_tracing::profile_scope!("left", format!("{} values", values.len()));
            self.data.extend(values);
        }
        {
            re_tracing::profile_scope!("right", format!("{} values", right.len()));
            self.data.extend(right);
        }

        // self.data = left.into_iter().chain(values).chain(right).collect();
        // TODO: figure out how costly => should be free if all inserts are in order, right?
        // TODO: also why on earth do we need this? ha, for iter_slices...
        self.data.make_contiguous();
    }
}

impl<T: Clone> FlatVecDeque<T> {
    pub fn to_vec(&self) -> Vec<T> {
        self.data.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extend_at() {
        let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

        assert_eq!(0, v.num_entries());
        assert_eq!(0, v.len());

        v.extend_at(0, [1, 2, 3].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3], v.to_vec());

        v.extend_at(0, [4, 5, 6, 7].into_iter());
        similar_asserts::assert_eq!(vec![4, 5, 6, 7, 1, 2, 3], v.to_vec());

        v.extend_at(0, [8, 9].into_iter());
        similar_asserts::assert_eq!(vec![8, 9, 4, 5, 6, 7, 1, 2, 3], v.to_vec());

        v.extend_at(0, std::iter::empty());
        similar_asserts::assert_eq!(vec![8, 9, 4, 5, 6, 7, 1, 2, 3], v.to_vec());

        v.extend_at(2, [10, 11, 12, 13].into_iter());
        similar_asserts::assert_eq!(vec![8, 9, 4, 5, 6, 7, 10, 11, 12, 13, 1, 2, 3], v.to_vec());

        v.extend_at(4, [14, 15].into_iter());
        similar_asserts::assert_eq!(
            vec![8, 9, 4, 5, 6, 7, 10, 11, 12, 13, 1, 2, 3, 14, 15],
            v.to_vec()
        );
    }

    #[test]
    fn extend_back_with() {
        let mut v1: FlatVecDeque<i64> = FlatVecDeque::new();
        let mut v2: FlatVecDeque<i64> = FlatVecDeque::new();

        assert_eq!(0, v1.num_entries());
        assert_eq!(0, v1.len());

        assert_eq!(0, v2.num_entries());
        assert_eq!(0, v2.len());

        v1.extend_at(0, [1, 2, 3].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3], v1.to_vec());

        v2.extend_at(0, [4, 5, 6, 7].into_iter());
        similar_asserts::assert_eq!(vec![4, 5, 6, 7], v2.to_vec());

        v1.extend_back_with(v2);
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7], v1.to_vec());

        let mut v3: FlatVecDeque<i64> = FlatVecDeque::new();
        v3.extend_back_with(v1);
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7], v3.to_vec());

        let mut v4: FlatVecDeque<i64> = FlatVecDeque::new();
        v4.extend_back([8]);
        similar_asserts::assert_eq!(vec![8], v4.to_vec());

        v3.extend_back_with(v4);
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7, 8], v3.to_vec());

        v3.extend_back_with(FlatVecDeque::new());
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7, 8], v3.to_vec());

        let mut v5 = FlatVecDeque::new();
        similar_asserts::assert_eq!(Vec::<i64>::new(), v5.to_vec());

        v5.extend_back_with(v3);
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7, 8], v5.to_vec());
    }

    #[test]
    fn remove_at() {
        let mut v: FlatVecDeque<i64> = FlatVecDeque::new();

        assert_eq!(0, v.num_entries());
        assert_eq!(0, v.len());

        v.extend_at(0, [1, 2, 3].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3], v.to_vec());

        v.remove_at(0);
        similar_asserts::assert_eq!(Vec::<i64>::new(), v.to_vec());

        v.extend_at(0, [1, 2, 3].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3], v.to_vec());

        v.extend_at(1, [4, 5, 6, 7].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7], v.to_vec());

        v.extend_at(2, [8, 9].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], v.to_vec());

        v.remove_at(0);
        similar_asserts::assert_eq!(vec![4, 5, 6, 7, 8, 9], v.to_vec());

        v.extend_at(0, [1, 2, 3].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], v.to_vec());

        v.remove_at(1);
        similar_asserts::assert_eq!(vec![1, 2, 3, 8, 9], v.to_vec());

        v.extend_at(1, [4, 5, 6, 7].into_iter());
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7, 8, 9], v.to_vec());

        v.remove_at(2);
        similar_asserts::assert_eq!(vec![1, 2, 3, 4, 5, 6, 7], v.to_vec());

        v.remove_at(0);
        similar_asserts::assert_eq!(vec![4, 5, 6, 7], v.to_vec());

        v.remove_at(0);
        similar_asserts::assert_eq!(Vec::<i64>::new(), v.to_vec());
    }
}
