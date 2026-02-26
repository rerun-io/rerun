use arrow::array::{Array as ArrowArray, ListArray as ArrowListArray};
use arrow::buffer::{OffsetBuffer as ArrowOffsets, ScalarBuffer as ArrowScalarBuffer};
use itertools::Itertools as _;
use re_log_types::TimelineName;

use crate::{Chunk, TimeColumn};

// ---

impl Chunk {
    /// Is the chunk currently ascendingly sorted by [`crate::RowId`]?
    ///
    /// This is O(1) (cached).
    ///
    /// See also [`Self::is_sorted_uncached`].
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.is_sorted
    }

    /// For debugging purposes.
    #[doc(hidden)]
    #[inline]
    pub fn is_sorted_uncached(&self) -> bool {
        re_tracing::profile_function!();

        self.row_ids()
            .tuple_windows::<(_, _)>()
            .all(|row_ids| row_ids.0 <= row_ids.1)
    }

    /// Is the chunk ascendingly sorted by time, for all of its timelines?
    ///
    /// This is O(1) (cached).
    #[inline]
    pub fn is_time_sorted(&self) -> bool {
        self.timelines
            .values()
            .all(|time_column| time_column.is_sorted())
    }

    /// Is the chunk ascendingly sorted by time, for a specific timeline?
    ///
    /// This is O(1) (cached).
    ///
    /// See also [`Self::is_timeline_sorted_uncached`].
    #[inline]
    pub fn is_timeline_sorted(&self, timeline: &TimelineName) -> bool {
        self.is_static()
            || self
                .timelines
                .get(timeline)
                .is_some_and(|time_column| time_column.is_sorted())
    }

    /// For debugging purposes.
    #[doc(hidden)]
    #[inline]
    pub fn is_timeline_sorted_uncached(&self, timeline: &TimelineName) -> bool {
        self.is_static()
            || self
                .timelines
                .get(timeline)
                .is_some_and(|time_column| time_column.is_sorted_uncached())
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

        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::Instant::now();

        let swaps = {
            re_tracing::profile_scope!("swaps");
            let row_ids = self.row_ids_slice();
            let mut swaps = (0..row_ids.len()).collect::<Vec<_>>();
            swaps.sort_by_key(|&i| row_ids[i]);
            swaps
        };

        self.shuffle_with(&swaps);

        #[cfg(not(target_arch = "wasm32"))]
        re_log::trace!(
            entity_path = %self.entity_path,
            num_rows = self.row_ids.len(),
            elapsed = ?now.elapsed(),
            "chunk sorted",
        );

        #[cfg(debug_assertions)]
        #[expect(clippy::unwrap_used)] // dev only
        self.sanity_check().unwrap();
    }

    /// Returns a new [`Chunk`] that is sorted by `(<timeline>, RowId)`.
    ///
    /// The underlying arrow data will be copied and shuffled in memory in order to make it contiguous.
    ///
    /// This is a no-op if the underlying timeline is already sorted appropriately (happy path).
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    pub fn sorted_by_timeline_if_unsorted(&self, timeline: &TimelineName) -> Self {
        let mut chunk = self.clone();

        let Some(time_column) = chunk.timelines.get(timeline) else {
            return chunk;
        };

        if time_column.is_sorted() {
            return chunk;
        }

        re_tracing::profile_function!();

        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::Instant::now();

        let swaps = {
            re_tracing::profile_scope!("swaps");
            let row_ids = chunk.row_ids_slice();
            let times = time_column.times_raw().to_vec();
            let mut swaps = (0..times.len()).collect::<Vec<_>>();
            swaps.sort_by_key(|&i| (times[i], row_ids[i]));
            swaps
        };

        chunk.shuffle_with(&swaps);

        #[cfg(not(target_arch = "wasm32"))]
        re_log::trace!(
            entity_path = %chunk.entity_path,
            num_rows = chunk.row_ids.len(),
            elapsed = ?now.elapsed(),
            "chunk sorted",
        );

        #[cfg(debug_assertions)]
        #[expect(clippy::unwrap_used)] // dev only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Randomly shuffles the chunk using the given `seed`.
    ///
    /// The underlying arrow data will be copied and shuffled in memory in order to make it contiguous.
    #[inline]
    pub fn shuffle_random(&mut self, seed: u64) {
        re_tracing::profile_function!();

        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::Instant::now();

        use rand::SeedableRng as _;
        use rand::seq::SliceRandom as _;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let swaps = {
            re_tracing::profile_scope!("swaps");
            let mut swaps = (0..self.row_ids.len()).collect::<Vec<_>>();
            swaps.shuffle(&mut rng);
            swaps
        };

        self.shuffle_with(&swaps);

        #[cfg(not(target_arch = "wasm32"))]
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
    // TODO(RR-3865): Use arrow's `ListView` to only shuffle offsets instead of the data itself.
    pub(crate) fn shuffle_with(&mut self, swaps: &[usize]) {
        re_tracing::profile_function!();

        // Row IDs
        {
            re_tracing::profile_scope!("row ids");

            let row_ids = self.row_ids_slice();

            let mut sorted_row_ids = row_ids.to_vec();
            for (to, from) in swaps.iter().copied().enumerate() {
                sorted_row_ids[to] = row_ids[from];
            }
            self.row_ids = re_types_core::RowId::arrow_from_slice(&sorted_row_ids);
        }

        let Self {
            id: _,
            entity_path: _,
            heap_size_bytes: _,
            is_sorted: _,
            row_ids: _,
            timelines,
            components,
        } = self;

        // Timelines
        {
            re_tracing::profile_scope!("timelines");

            for info in timelines.values_mut() {
                let TimeColumn {
                    timeline: _,
                    times,
                    is_sorted,
                    time_range: _,
                } = info;

                let mut sorted = times.to_vec();
                for (to, from) in swaps.iter().copied().enumerate() {
                    sorted[to] = times[from];
                }

                *is_sorted = sorted.windows(2).all(|times| times[0] <= times[1]);
                *times = ArrowScalarBuffer::from(sorted);
            }
        }

        // Components
        //
        // Reminder: these are all `ListArray`s.
        re_tracing::profile_scope!("components (offsets & data)");
        {
            for original in components.list_arrays_mut() {
                let sorted_arrays = swaps
                    .iter()
                    .copied()
                    .map(|from| original.value(from))
                    .collect_vec();
                let sorted_arrays = sorted_arrays
                    .iter()
                    .map(|array| &**array as &dyn ArrowArray)
                    .collect_vec();

                let datatype = original.data_type().clone();
                let offsets =
                    ArrowOffsets::from_lengths(sorted_arrays.iter().map(|array| array.len()));
                #[expect(clippy::unwrap_used)] // these are slices of the same outer array
                let values = re_arrow_util::concat_arrays(&sorted_arrays).unwrap();
                let validity = original
                    .nulls()
                    .map(|validity| swaps.iter().map(|&from| validity.is_valid(from)).collect());

                let field = match datatype {
                    arrow::datatypes::DataType::List(field) => field.clone(),
                    _ => unreachable!("This is always s list array"),
                };
                *original = ArrowListArray::new(field, offsets, values, validity);
            }
        }

        self.is_sorted = self.is_sorted_uncached();
    }
}

impl TimeColumn {
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
        self.times_raw()
            .windows(2)
            .all(|times| times[0] <= times[1])
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{EntityPath, Timeline};

    use super::*;
    use crate::{ChunkId, RowId};

    #[test]
    fn sort() -> anyhow::Result<()> {
        let entity_path: EntityPath = "a/b/c".into();

        let timeline1 = Timeline::new_duration("log_time");
        let timeline2 = Timeline::new_sequence("frame_nr");

        let points1 = vec![
            MyPoint::new(1.0, 2.0),
            MyPoint::new(3.0, 4.0),
            MyPoint::new(5.0, 6.0),
        ];
        let points3 = vec![MyPoint::new(10.0, 20.0)];
        let points4 = vec![MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)];

        let colors1 = vec![
            MyColor::from_rgb(1, 2, 3),
            MyColor::from_rgb(4, 5, 6),
            MyColor::from_rgb(7, 8, 9),
        ];
        let colors2 = vec![MyColor::from_rgb(10, 20, 30)];
        let colors4 = vec![
            MyColor::from_rgb(101, 102, 103),
            MyColor::from_rgb(104, 105, 106),
        ];

        {
            let chunk_sorted = Chunk::builder(entity_path.clone())
                .with_sparse_component_batches(
                    RowId::new(),
                    [(timeline1, 1000), (timeline2, 42)],
                    [
                        (MyPoints::descriptor_points(), Some(&points1 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors1 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    RowId::new(),
                    [(timeline1, 1001), (timeline2, 43)],
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(&colors2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    RowId::new(),
                    [(timeline1, 1002), (timeline2, 44)],
                    [
                        (MyPoints::descriptor_points(), Some(&points3 as _)),
                        (MyPoints::descriptor_colors(), None),
                    ],
                )
                .with_sparse_component_batches(
                    RowId::new(),
                    [(timeline1, 1003), (timeline2, 45)],
                    [
                        (MyPoints::descriptor_points(), Some(&points4 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors4 as _)),
                    ],
                )
                .build()?;

            eprintln!("{chunk_sorted}");

            assert!(chunk_sorted.is_sorted());
            assert!(chunk_sorted.is_sorted_uncached());

            let chunk_shuffled = {
                let mut chunk_shuffled = chunk_sorted.clone();
                chunk_shuffled.shuffle_random(666);
                chunk_shuffled
            };

            eprintln!("{chunk_shuffled}");

            assert!(!chunk_shuffled.is_sorted());
            assert!(!chunk_shuffled.is_sorted_uncached());
            assert_ne!(chunk_sorted, chunk_shuffled);

            let chunk_resorted = {
                let mut chunk_resorted = chunk_shuffled.clone();
                chunk_resorted.sort_if_unsorted();
                chunk_resorted
            };

            eprintln!("{chunk_resorted}");

            assert!(chunk_resorted.is_sorted());
            assert!(chunk_resorted.is_sorted_uncached());
            assert_eq!(chunk_sorted, chunk_resorted);
        }

        Ok(())
    }

    #[test]
    fn sort_time() -> anyhow::Result<()> {
        let entity_path: EntityPath = "a/b/c".into();

        let timeline1 = Timeline::new_duration("log_time");
        let timeline2 = Timeline::new_sequence("frame_nr");

        let chunk_id = ChunkId::new();
        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();

        let points1 = vec![
            MyPoint::new(1.0, 2.0),
            MyPoint::new(3.0, 4.0),
            MyPoint::new(5.0, 6.0),
        ];
        let points3 = vec![MyPoint::new(10.0, 20.0)];
        let points4 = vec![MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)];

        let colors1 = vec![
            MyColor::from_rgb(1, 2, 3),
            MyColor::from_rgb(4, 5, 6),
            MyColor::from_rgb(7, 8, 9),
        ];
        let colors2 = vec![MyColor::from_rgb(10, 20, 30)];
        let colors4 = vec![
            MyColor::from_rgb(101, 102, 103),
            MyColor::from_rgb(104, 105, 106),
        ];

        {
            let chunk_unsorted_timeline2 = Chunk::builder_with_id(chunk_id, entity_path.clone())
                .with_sparse_component_batches(
                    row_id1,
                    [(timeline1, 1000), (timeline2, 45)],
                    [
                        (MyPoints::descriptor_points(), Some(&points1 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors1 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id2,
                    [(timeline1, 1001), (timeline2, 44)],
                    [
                        (MyPoints::descriptor_points(), None),
                        (MyPoints::descriptor_colors(), Some(&colors2 as _)),
                    ],
                )
                .with_sparse_component_batches(
                    row_id3,
                    [(timeline1, 1002), (timeline2, 43)],
                    [
                        (MyPoints::descriptor_points(), Some(&points3 as _)),
                        (MyPoints::descriptor_colors(), None),
                    ],
                )
                .with_sparse_component_batches(
                    row_id4,
                    [(timeline1, 1003), (timeline2, 42)],
                    [
                        (MyPoints::descriptor_points(), Some(&points4 as _)),
                        (MyPoints::descriptor_colors(), Some(&colors4 as _)),
                    ],
                )
                .build()?;

            eprintln!("unsorted:\n{chunk_unsorted_timeline2}");

            assert!(chunk_unsorted_timeline2.is_sorted());
            assert!(chunk_unsorted_timeline2.is_sorted_uncached());

            assert!(
                chunk_unsorted_timeline2
                    .timelines()
                    .get(timeline1.name())
                    .unwrap()
                    .is_sorted()
            );
            assert!(
                chunk_unsorted_timeline2
                    .timelines()
                    .get(timeline1.name())
                    .unwrap()
                    .is_sorted_uncached()
            );

            assert!(
                !chunk_unsorted_timeline2
                    .timelines()
                    .get(timeline2.name())
                    .unwrap()
                    .is_sorted()
            );
            assert!(
                !chunk_unsorted_timeline2
                    .timelines()
                    .get(timeline2.name())
                    .unwrap()
                    .is_sorted_uncached()
            );

            let chunk_sorted_timeline2 =
                chunk_unsorted_timeline2.sorted_by_timeline_if_unsorted(timeline2.name());

            eprintln!("sorted:\n{chunk_sorted_timeline2}");

            assert!(!chunk_sorted_timeline2.is_sorted());
            assert!(!chunk_sorted_timeline2.is_sorted_uncached());

            assert!(
                !chunk_sorted_timeline2
                    .timelines()
                    .get(timeline1.name())
                    .unwrap()
                    .is_sorted()
            );
            assert!(
                !chunk_sorted_timeline2
                    .timelines()
                    .get(timeline1.name())
                    .unwrap()
                    .is_sorted_uncached()
            );

            assert!(
                chunk_sorted_timeline2
                    .timelines()
                    .get(timeline2.name())
                    .unwrap()
                    .is_sorted()
            );
            assert!(
                chunk_sorted_timeline2
                    .timelines()
                    .get(timeline2.name())
                    .unwrap()
                    .is_sorted_uncached()
            );

            let chunk_sorted_timeline2_expected =
                Chunk::builder_with_id(chunk_id, entity_path.clone())
                    .with_sparse_component_batches(
                        row_id4,
                        [(timeline1, 1003), (timeline2, 42)],
                        [
                            (MyPoints::descriptor_points(), Some(&points4 as _)),
                            (MyPoints::descriptor_colors(), Some(&colors4 as _)),
                        ],
                    )
                    .with_sparse_component_batches(
                        row_id3,
                        [(timeline1, 1002), (timeline2, 43)],
                        [
                            (MyPoints::descriptor_points(), Some(&points3 as _)),
                            (MyPoints::descriptor_colors(), None),
                        ],
                    )
                    .with_sparse_component_batches(
                        row_id2,
                        [(timeline1, 1001), (timeline2, 44)],
                        [
                            (MyPoints::descriptor_points(), None),
                            (MyPoints::descriptor_colors(), Some(&colors2 as _)),
                        ],
                    )
                    .with_sparse_component_batches(
                        row_id1,
                        [(timeline1, 1000), (timeline2, 45)],
                        [
                            (MyPoints::descriptor_points(), Some(&points1 as _)),
                            (MyPoints::descriptor_colors(), Some(&colors1 as _)),
                        ],
                    )
                    .build()?;

            eprintln!("expected:\n{chunk_sorted_timeline2}");

            assert_eq!(
                chunk_sorted_timeline2_expected,
                chunk_sorted_timeline2,
                "{}",
                similar_asserts::SimpleDiff::from_str(
                    &format!("{chunk_sorted_timeline2_expected}"),
                    &format!("{chunk_sorted_timeline2}"),
                    "got",
                    "expected",
                ),
            );
        }

        Ok(())
    }
}
