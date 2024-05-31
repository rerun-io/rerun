use arrow2::{
    array::{Array as ArrowArray, ListArray as ArrowListArray},
    offset::Offsets as ArrowOffsets,
};
use itertools::Itertools as _;

use crate::{Chunk, ChunkTimeline};

// ---

impl Chunk {
    /// Is the chunk currently ascendingly sorted by [`re_log_types::RowId`]?
    ///
    /// This is O(1) (cached).
    ///
    /// See also [`Self::is_sorted_uncached`].
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.is_sorted
    }

    /// Like [`Self::is_sorted`], but actually checks the entire dataset rather than relying on the
    /// cached value.
    ///
    /// O(n). Useful for tests/debugging, or when you just don't know.
    ///
    /// See also [`Self::is_sorted`].
    #[inline]
    pub fn is_sorted_uncached(&self) -> bool {
        re_tracing::profile_function!();

        self.row_ids
            .windows(2)
            .all(|row_ids| row_ids[0] <= row_ids[1])
    }

    /// Sort the chunk, if needed.
    ///
    /// The underlying arrow data will be copied and shuffled in memory in order to make it contiguous.
    #[inline]
    pub fn sort_if_unsorted(&mut self) {
        if self.is_sorted() {
            return;
        }

        re_tracing::profile_function!();

        let now = std::time::Instant::now();

        let swaps = {
            re_tracing::profile_scope!("swaps");
            let mut swaps = (0..self.row_ids.len()).collect::<Vec<_>>();
            swaps.sort_by_key(|&i| self.row_ids[i]);
            swaps
        };

        self.shuffle_with(&swaps);

        re_log::trace!(
            entity_path = %self.entity_path,
            num_rows = self.row_ids.len(),
            elapsed = ?now.elapsed(),
            "chunk sorted",
        );

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // dev only
        self.sanity_check().unwrap();
    }

    /// Randomly shuffles the chunk using the given `seed`.
    ///
    /// The underlying arrow data will be copied and shuffled in memory in order to make it contiguous.
    #[inline]
    pub fn shuffle_random(&mut self, seed: u64) {
        re_tracing::profile_function!();

        let now = std::time::Instant::now();

        use rand::{seq::SliceRandom as _, SeedableRng as _};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let swaps = {
            re_tracing::profile_scope!("swaps");
            let mut swaps = (0..self.row_ids.len()).collect::<Vec<_>>();
            swaps.shuffle(&mut rng);
            swaps
        };

        self.shuffle_with(&swaps);

        re_log::trace!(
            entity_path = %self.entity_path,
            num_rows = self.row_ids.len(),
            elapsed = ?now.elapsed(),
            "chunk shuffled",
        );
    }

    /// Shuffle the chunk according to the specified `swaps`.
    ///
    /// `swaps` is a slice that maps an implicit destination index to its explicit source index.
    /// E.g. `swap[0] = 3` means that the entry at index `3` in the original chunk should be move to index `0`.
    ///
    /// The underlying arrow data will be copied and shuffled in memory in order to make it contiguous.
    //
    // TODO(#3741): Provide a path that only shuffles offsets instead of the data itself, using a `ListView`.
    pub(crate) fn shuffle_with(&mut self, swaps: &[usize]) {
        re_tracing::profile_function!();

        let Self {
            id: _,
            entity_path: _,
            is_sorted: _,
            row_ids,
            timelines,
            components,
        } = self;

        // Row IDs
        {
            re_tracing::profile_scope!("row ids");

            let original = row_ids.clone();
            for (to, from) in swaps.iter().copied().enumerate() {
                row_ids[to] = original[from];
            }
        }

        // Timelines
        {
            re_tracing::profile_scope!("timelines");

            for info in timelines.values_mut() {
                let ChunkTimeline {
                    times,
                    is_sorted,
                    time_range: _,
                } = info;

                let original = times.clone();
                for (to, from) in swaps.iter().copied().enumerate() {
                    times[to] = original[from];
                }

                *is_sorted = times.windows(2).all(|times| times[0] <= times[1]);
            }
        }

        // Components
        //
        // Reminder: these are all `ListArray`s.
        re_tracing::profile_scope!("components (offsets & data)");
        {
            for original in components.values_mut() {
                #[allow(clippy::unwrap_used)] // a chunk's column is always a list array
                let original_list = original
                    .as_any()
                    .downcast_ref::<ArrowListArray<i32>>()
                    .unwrap();

                let sorted_arrays = swaps
                    .iter()
                    .copied()
                    .map(|from| original_list.value(from))
                    .collect_vec();
                let sorted_arrays = sorted_arrays
                    .iter()
                    .map(|array| &**array as &dyn ArrowArray)
                    .collect_vec();

                let datatype = original.data_type().clone();
                #[allow(clippy::unwrap_used)] // yep, these are in fact lengths
                let offsets =
                    ArrowOffsets::try_from_lengths(sorted_arrays.iter().map(|array| array.len()))
                        .unwrap();
                #[allow(clippy::unwrap_used)] // these are slices of the same outer array
                let values = arrow2::compute::concatenate::concatenate(&sorted_arrays).unwrap();
                let validity = original_list
                    .validity()
                    .map(|validity| swaps.iter().map(|&from| validity.get_bit(from)).collect());

                *original =
                    ArrowListArray::<i32>::new(datatype, offsets.into(), values, validity).boxed();
            }
        }

        self.is_sorted = self.is_sorted_uncached();
    }
}

impl ChunkTimeline {
    /// Is the timeline sorted?
    ///
    /// This is O(1) (cached).
    ///
    /// See also [`Self::is_sorted_uncached`].
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.is_sorted
    }

    /// Like [`Self::is_sorted`], but actually checks the entire dataset rather than relying on the
    /// cached value.
    ///
    /// O(n). Useful for tests/debugging, or when you just don't know.
    ///
    /// See also [`Self::is_sorted`].
    #[inline]
    pub fn is_sorted_uncached(&self) -> bool {
        re_tracing::profile_function!();
        self.times.windows(2).all(|times| times[0] <= times[1])
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::{
        example_components::{MyColor, MyPoint},
        EntityPath, RowId, TimeInt, Timeline,
    };
    use re_types_core::Loggable as _;

    use crate::{arrays_to_list_array, ChunkId};

    use super::*;

    #[test]
    fn sort() -> anyhow::Result<()> {
        let entity_path: EntityPath = "a/b/c".into();

        let timeline1 = Timeline::new_temporal("log_time");
        let timeline2 = Timeline::new_temporal("frame_nr");

        let points1 = MyPoint::to_arrow([
            MyPoint::new(1.0, 2.0),
            MyPoint::new(3.0, 4.0),
            MyPoint::new(5.0, 6.0),
        ])?;
        let points2 = None;
        let points3 = MyPoint::to_arrow([MyPoint::new(10.0, 20.0)])?;
        let points4 = MyPoint::to_arrow([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;

        let colors1 = MyColor::to_arrow([
            MyColor::from_rgb(1, 2, 3),
            MyColor::from_rgb(4, 5, 6),
            MyColor::from_rgb(7, 8, 9),
        ])?;
        let colors2 = MyColor::to_arrow([MyColor::from_rgb(10, 20, 30)])?;
        let colors3 = None;
        let colors4 = MyColor::to_arrow([
            MyColor::from_rgb(101, 102, 103),
            MyColor::from_rgb(104, 105, 106),
        ])?;

        let timelines = [
            (
                timeline1,
                ChunkTimeline::new(
                    Some(true),
                    [1000, 1001, 1002, 1003].map(TimeInt::new_temporal).to_vec(),
                )
                .unwrap(),
            ),
            (
                timeline2,
                ChunkTimeline::new(
                    Some(true),
                    [42, 43, 44, 45].map(TimeInt::new_temporal).to_vec(),
                )
                .unwrap(),
            ),
        ];

        let components = [
            (
                MyPoint::name(),
                arrays_to_list_array(&[Some(&*points1), points2, Some(&*points3), Some(&*points4)])
                    .unwrap(),
            ),
            (
                MyPoint::name(),
                arrays_to_list_array(&[Some(&*colors1), Some(&*colors2), colors3, Some(&*colors4)])
                    .unwrap(),
            ),
        ];

        let row_ids = vec![RowId::new(), RowId::new(), RowId::new(), RowId::new()];

        {
            let chunk_sorted = Chunk::new(
                ChunkId::new(),
                entity_path.clone(),
                Some(true),
                row_ids.clone(),
                timelines.clone().into_iter().collect(),
                components.clone().into_iter().collect(),
            )?;

            // eprintln!("{chunk_sorted}");

            assert!(chunk_sorted.is_sorted());
            assert!(chunk_sorted.is_sorted_uncached());

            let chunk_shuffled = {
                let mut chunk_shuffled = chunk_sorted.clone();
                chunk_shuffled.shuffle_random(666);
                chunk_shuffled
            };

            // eprintln!("{chunk_shuffled}");

            assert!(!chunk_shuffled.is_sorted());
            assert!(!chunk_shuffled.is_sorted_uncached());
            assert_ne!(chunk_sorted, chunk_shuffled);

            let chunk_resorted = {
                let mut chunk_resorted = chunk_shuffled.clone();
                chunk_resorted.sort_if_unsorted();
                chunk_resorted
            };

            // eprintln!("{chunk_resorted}");

            assert!(chunk_resorted.is_sorted());
            assert!(chunk_resorted.is_sorted_uncached());
            assert_eq!(chunk_sorted, chunk_resorted);
        }

        Ok(())
    }
}
