use arrow2::array::{
    Array as ArrowArray, BooleanArray as ArrowBooleanArray, ListArray,
    PrimitiveArray as ArrowPrimitiveArray, StructArray,
};

use itertools::Itertools;
use nohash_hasher::IntSet;
use re_log_types::Timeline;
use re_types_core::ComponentName;

use crate::{Chunk, ChunkTimeline};

// ---

// NOTE: Not worth writing tests for all of these, until some subtle bug comes up.
// Most of them are indirectly stressed by our higher-level query tests anyhow.

impl Chunk {
    /// Slices the [`Chunk`] vertically.
    ///
    /// The result is a new [`Chunk`] with the same columns and (potentially) less rows.
    ///
    /// This cannot fail nor panic: `index` and `len` will be capped so that they cannot
    /// run out of bounds.
    /// This can result in an empty [`Chunk`] being returned if the slice is completely OOB.
    #[inline]
    pub fn row_sliced(&self, index: usize, len: usize) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        // NOTE: Bound checking costs are completely dwarfed by everything else, and preventing the
        // viewer from crashing is more important than anything else in any case.

        if index >= self.num_rows() {
            return self.emptied();
        }

        let end_offset = usize::min(index.saturating_add(len), self.num_rows());
        let len = end_offset.saturating_sub(index);

        if len == 0 {
            return self.emptied();
        }

        let is_sorted = *is_sorted || (len < 2);

        let mut chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted,
            row_ids: row_ids.clone().sliced(index, len),
            timelines: timelines
                .iter()
                .map(|(timeline, time_chunk)| (*timeline, time_chunk.row_sliced(index, len)))
                .collect(),
            components: components
                .iter()
                .map(|(component_name, list_array)| {
                    (*component_name, list_array.clone().sliced(index, len))
                })
                .collect(),
        };

        // We can know for sure whether the resulting chunk is already sorted (see conditional
        // above), but the reverse is not true.
        //
        // Consider e.g. slicing the following chunk on `(1..=3)`:
        // ┌──────────────┬───────────────────┬────────────────────────────────────────────┐
        // │ frame        ┆ example.MyColor   ┆ example.MyPoint                            │
        // ╞══════════════╪═══════════════════╪════════════════════════════════════════════╡
        // │ 3            ┆ [4278255873]      ┆ -                                          │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 1            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 2            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 3            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 5            ┆ -                 ┆ [{x: 3, y: 3}, {x: 4, y: 4}, {x: 5, y: 5}] │
        // └──────────────┴───────────────────┴────────────────────────────────────────────┘
        //
        // The original chunk is unsorted, but the new sliced one actually ends up being sorted.
        chunk.is_sorted = is_sorted || chunk.is_sorted_uncached();

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Slices the [`Chunk`] horizontally by keeping only the selected `timeline`.
    ///
    /// The result is a new [`Chunk`] with the same rows and (at-most) one timeline column.
    /// All non-timeline columns will be kept as-is.
    ///
    /// If `timeline` is not found within the [`Chunk`], the end result will be the same as the
    /// current chunk but without any timeline column.
    #[inline]
    pub fn timeline_sliced(&self, timeline: Timeline) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted: *is_sorted,
            row_ids: row_ids.clone(),
            timelines: timelines
                .get_key_value(&timeline)
                .map(|(timeline, time_chunk)| (*timeline, time_chunk.clone()))
                .into_iter()
                .collect(),
            components: components.clone(),
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Slices the [`Chunk`] horizontally by keeping only the selected `component_name`.
    ///
    /// The result is a new [`Chunk`] with the same rows and (at-most) one component column.
    /// All non-component columns will be kept as-is.
    ///
    /// If `component_name` is not found within the [`Chunk`], the end result will be the same as the
    /// current chunk but without any component column.
    #[inline]
    pub fn component_sliced(&self, component_name: ComponentName) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted: *is_sorted,
            row_ids: row_ids.clone(),
            timelines: timelines.clone(),
            components: components
                .get_key_value(&component_name)
                .map(|(component_name, list_array)| (*component_name, list_array.clone()))
                .into_iter()
                .collect(),
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Slices the [`Chunk`] horizontally by keeping only the selected timelines.
    ///
    /// The result is a new [`Chunk`] with the same rows and (at-most) the selected timeline columns.
    /// All non-timeline columns will be kept as-is.
    ///
    /// If none of the selected timelines exist in the [`Chunk`], the end result will be the same as the
    /// current chunk but without any timeline column.
    #[inline]
    pub fn timelines_sliced(&self, timelines_to_keep: &IntSet<Timeline>) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted: *is_sorted,
            row_ids: row_ids.clone(),
            timelines: timelines
                .iter()
                .filter(|(timeline, _)| timelines_to_keep.contains(timeline))
                .map(|(timeline, time_chunk)| (*timeline, time_chunk.clone()))
                .collect(),
            components: components.clone(),
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Slices the [`Chunk`] horizontally by keeping only the selected `component_names`.
    ///
    /// The result is a new [`Chunk`] with the same rows and (at-most) the selected component columns.
    /// All non-component columns will be kept as-is.
    ///
    /// If none of the `component_names` exist in the [`Chunk`], the end result will be the same as the
    /// current chunk but without any component column.
    #[inline]
    pub fn components_sliced(&self, component_names: &IntSet<ComponentName>) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted: *is_sorted,
            row_ids: row_ids.clone(),
            timelines: timelines.clone(),
            components: components
                .iter()
                .filter(|(component_name, _)| component_names.contains(component_name))
                .map(|(component_name, list_array)| (*component_name, list_array.clone()))
                .collect(),
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Densifies the [`Chunk`] vertically based on the `component_name` column.
    ///
    /// Densifying here means dropping all rows where the associated value in the `component_name`
    /// column is null.
    ///
    /// The result is a new [`Chunk`] where the `component_name` column is guaranteed to be dense.
    ///
    /// If `component_name` doesn't exist in this [`Chunk`], or if it is already dense, this method
    /// is a no-op.
    #[inline]
    pub fn densified(&self, component_name: ComponentName) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        if self.is_empty() {
            return self.clone();
        }

        let Some(component_list_array) = components.get(&component_name) else {
            return self.clone();
        };

        let Some(validity) = component_list_array.validity() else {
            return self.clone();
        };

        let mask = validity.iter().collect_vec();
        let is_sorted = *is_sorted || (mask.iter().filter(|&&b| b).count() < 2);
        let validity_filter = ArrowBooleanArray::from_slice(mask);

        let mut chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted,
            row_ids: crate::util::filter_array(row_ids, &validity_filter),
            timelines: timelines
                .iter()
                .map(|(&timeline, time_chunk)| (timeline, time_chunk.filtered(&validity_filter)))
                .collect(),
            components: components
                .iter()
                // TODO: for the component in question, this should drop the bitmap, not filter it.
                // tests should make sure that this is the case, too.
                .map(|(&component_name, list_array)| {
                    (
                        component_name,
                        crate::util::filter_array(list_array, &validity_filter),
                    )
                })
                .collect(),
        };

        // We can know for sure whether the resulting chunk is already sorted (see conditional
        // above), but the reverse is not true.
        //
        // Consider e.g. densifying the following chunk on `example.MyPoint`:
        // ┌──────────────┬───────────────────┬────────────────────────────────────────────┐
        // │ frame        ┆ example.MyColor   ┆ example.MyPoint                            │
        // ╞══════════════╪═══════════════════╪════════════════════════════════════════════╡
        // │ 3            ┆ [4278255873]      ┆ -                                          │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 1            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 5            ┆ -                 ┆ [{x: 3, y: 3}, {x: 4, y: 4}, {x: 5, y: 5}] │
        // └──────────────┴───────────────────┴────────────────────────────────────────────┘
        //
        // The original chunk is unsorted, but the new filtered one actually ends up being sorted.
        chunk.is_sorted = is_sorted || chunk.is_sorted_uncached();

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Empties the [`Chunk`] vertically.
    ///
    /// The result is a new [`Chunk`] with the same columns but zero rows.
    #[inline]
    pub fn emptied(&self) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted: _,
            row_ids,
            timelines,
            components,
        } = self;

        Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted: true,
            row_ids: StructArray::new_empty(row_ids.data_type().clone()),
            timelines: timelines
                .iter()
                .map(|(&timeline, time_chunk)| (timeline, time_chunk.emptied()))
                .collect(),
            components: components
                .iter()
                .map(|(&component_name, list_array)| {
                    (
                        component_name,
                        ListArray::new_empty(list_array.data_type().clone()),
                    )
                })
                .collect(),
        }
    }
}

impl ChunkTimeline {
    /// Slices the [`ChunkTimeline`] vertically.
    ///
    /// The result is a new [`ChunkTimeline`] with the same timelines and (potentially) less rows.
    ///
    /// This cannot fail nor panic: `index` and `len` will be capped so that they cannot
    /// run out of bounds.
    /// This can result in an empty [`ChunkTimeline`] being returned if the slice is completely OOB.
    #[inline]
    pub fn row_sliced(&self, index: usize, len: usize) -> Self {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range: _,
        } = self;

        // NOTE: Bound checking costs are completely dwarfed by everything else, and preventing the
        // viewer from crashing is more important than anything else in any case.

        if index >= self.num_rows() {
            return self.emptied();
        }

        let end_offset = usize::min(index.saturating_add(len), self.num_rows());
        let len = end_offset.saturating_sub(index);

        if len == 0 {
            return self.emptied();
        }

        let is_sorted = *is_sorted || (len < 2);

        // We can know for sure whether the resulting chunk is already sorted (see conditional
        // above), but the reverse is not true.
        //
        // Consider e.g. slicing the following chunk on `(1..=3)`:
        // ┌──────────────┬───────────────────┬────────────────────────────────────────────┐
        // │ frame        ┆ example.MyColor   ┆ example.MyPoint                            │
        // ╞══════════════╪═══════════════════╪════════════════════════════════════════════╡
        // │ 3            ┆ [4278255873]      ┆ -                                          │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 1            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 2            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 3            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 5            ┆ -                 ┆ [{x: 3, y: 3}, {x: 4, y: 4}, {x: 5, y: 5}] │
        // └──────────────┴───────────────────┴────────────────────────────────────────────┘
        //
        // The original chunk is unsorted, but the new sliced one actually ends up being sorted.
        let is_sorted_opt = is_sorted.then_some(is_sorted);

        Self::new(
            is_sorted_opt,
            *timeline,
            ArrowPrimitiveArray::sliced(times.clone(), index, len),
        )
    }

    /// Empties the [`ChunkTimeline`] vertically.
    ///
    /// The result is a new [`ChunkTimeline`] with the same columns but zero rows.
    #[inline]
    pub fn emptied(&self) -> Self {
        let Self {
            timeline,
            times,
            is_sorted: _,
            time_range: _,
        } = self;

        Self::new(
            Some(true),
            *timeline,
            ArrowPrimitiveArray::new_empty(times.data_type().clone()),
        )
    }

    /// Runs a filter compute kernel on the time data with the specified `mask`.
    #[inline]
    pub(crate) fn filtered(&self, filter: &ArrowBooleanArray) -> Self {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range: _,
        } = self;

        let is_sorted = *is_sorted || filter.values_iter().filter(|&b| b).count() < 2;

        // We can know for sure whether the resulting chunk is already sorted (see conditional
        // above), but the reverse is not true.
        //
        // Consider e.g. densifying the following chunk on `example.MyPoint`:
        // ┌──────────────┬───────────────────┬────────────────────────────────────────────┐
        // │ frame        ┆ example.MyColor   ┆ example.MyPoint                            │
        // ╞══════════════╪═══════════════════╪════════════════════════════════════════════╡
        // │ 3            ┆ [4278255873]      ┆ -                                          │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 1            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 5            ┆ -                 ┆ [{x: 3, y: 3}, {x: 4, y: 4}, {x: 5, y: 5}] │
        // └──────────────┴───────────────────┴────────────────────────────────────────────┘
        //
        // The original chunk is unsorted, but the new filtered one actually ends up being sorted.
        let is_sorted_opt = is_sorted.then_some(is_sorted);

        Self::new(
            is_sorted_opt,
            *timeline,
            crate::util::filter_array(times, filter),
        )
    }
}
