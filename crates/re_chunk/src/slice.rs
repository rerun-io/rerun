use arrow2::array::{
    Array as ArrowArray, BooleanArray as ArrowBooleanArray, ListArray,
    PrimitiveArray as ArrowPrimitiveArray, StructArray,
};

use itertools::Itertools;
use nohash_hasher::IntSet;
use re_log_types::{RowId, TimeInt, Timeline};
use re_types_core::ComponentName;

use crate::{Chunk, ChunkTimeline};

// ---

// TODO: i really don't like all the panics in there, we need to manually check
// TODO: be wary when implementing something that looks suspiciously like a compute kernel, wink wink.
// TODO: tested indirectly by all our query tests -- not worth testing invidividually.

// TODO: tests or not?

impl Chunk {
    /// Slices the [`Chunk`] vertically.
    ///
    /// The result is a new [`Chunk`] with the same columns and (potentially) less rows.
    ///
    /// # Panics
    ///
    /// * If `index + len > self.num_rows()`.
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

        if len == 0 {
            return self.emptied();
        }

        let is_sorted = *is_sorted || (len < 2);

        let chunk = Self {
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
                .map(|(&component_name, list_array)| {
                    (
                        component_name,
                        crate::util::filter_array(list_array, &validity_filter),
                    )
                })
                .collect(),
        };

        // TODO: explain how this can sometimes be necessary
        //
        // E.g. imagine densifying the following chunk on `example.MyPoint`:
        // ┌──────────────┬───────────────────┬────────────────────────────────────────────┐
        // │ frame        ┆ example.MyColor   ┆ example.MyPoint                            │
        // ╞══════════════╪═══════════════════╪════════════════════════════════════════════╡
        // │ 3            ┆ [4278255873]      ┆ -                                          │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 1            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 5            ┆ -                 ┆ [{x: 3, y: 3}, {x: 4, y: 4}, {x: 5, y: 5}] │
        // └──────────────┴───────────────────┴────────────────────────────────────────────┘
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

// TODO: sorting a chunk by timeline will be hugely important too

impl ChunkTimeline {
    /// Slices the [`ChunkTimeline`] vertically.
    ///
    /// The result is a new [`Chunk`] with the same columns and (potentially) less rows.
    ///
    /// # Panics
    ///
    /// * If `index + len > self.num_rows()`.
    #[inline]
    pub fn row_sliced(&self, index: usize, len: usize) -> Self {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range: _,
        } = self;

        if len == 0 {
            return self.emptied();
        }

        let is_sorted = *is_sorted || (len < 2);

        Self::new(
            Some(is_sorted),
            *timeline,
            ArrowPrimitiveArray::sliced(times.clone() /* cheap */, index, len),
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

    // TODO
    #[inline]
    pub(crate) fn filtered(&self, filter: &ArrowBooleanArray) -> Self {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range: _,
        } = self;

        let is_sorted = *is_sorted || filter.values_iter().filter(|&b| b).count() < 2;

        // TODO: explain how this can happen to be necessary
        //
        // E.g. imagine densifying the following chunk on `example.MyPoint`:
        // ┌──────────────┬───────────────────┬────────────────────────────────────────────┐
        // │ frame        ┆ example.MyColor   ┆ example.MyPoint                            │
        // ╞══════════════╪═══════════════════╪════════════════════════════════════════════╡
        // │ 3            ┆ [4278255873]      ┆ -                                          │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 1            ┆ -                 ┆ [{x: 1, y: 1}, {x: 2, y: 2}]               │
        // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        // │ 5            ┆ -                 ┆ [{x: 3, y: 3}, {x: 4, y: 4}, {x: 5, y: 5}] │
        // └──────────────┴───────────────────┴────────────────────────────────────────────┘
        let is_sorted_opt = is_sorted.then_some(is_sorted);

        Self::new(
            is_sorted_opt,
            *timeline,
            crate::util::filter_array(times, filter),
        )
    }
}

// ---

// TODO: iteration doesn't really belong here though

// TODO: we'll do a better in the future -- for now it doesn't matter as it's just a way of feeding
// the cache anyhow.

impl Chunk {
    pub fn iter(
        &self,
        timeline: &Timeline,
        component_name: &ComponentName,
    ) -> impl Iterator<Item = (TimeInt, RowId, Option<Box<dyn ArrowArray>>)> + '_ {
        let Self {
            id: _,
            entity_path: _,
            heap_size_bytes: _,
            is_sorted: _,
            row_ids: _,
            timelines,
            components,
        } = self;

        let row_ids = self.row_ids();

        let data_times = timelines
            .get(timeline)
            .into_iter()
            .flat_map(|time_chunk| {
                time_chunk
                    .times
                    .values_iter()
                    .copied()
                    .map(TimeInt::new_temporal)
                    .collect::<Vec<_>>()
            })
            // TODO: explain
            .chain(std::iter::repeat(TimeInt::STATIC));

        let arrays = components
            .get(component_name)
            .into_iter()
            .flat_map(|list_array| list_array.into_iter().collect::<Vec<_>>()); // TODO

        itertools::izip!(data_times, row_ids, arrays)
    }
}
