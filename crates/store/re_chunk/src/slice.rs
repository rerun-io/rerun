use arrow::array::{
    Array as _, ArrayRef as ArrowArrayRef, BooleanArray as ArrowBooleanArray,
    ListArray as ArrowListArray,
};
use itertools::Itertools as _;
use nohash_hasher::IntSet;

use re_log_types::TimelineName;
use re_types_core::ComponentDescriptor;

use crate::{Chunk, RowId, TimeColumn};

// ---

// NOTE: Not worth writing tests for all of these, until some subtle bug comes up.
// Most of them are indirectly stressed by our higher-level query tests anyhow.

impl Chunk {
    /// Returns the cell corresponding to the specified [`RowId`] for a given [`re_types_core::ComponentName`].
    ///
    /// This is `O(log(n))` if `self.is_sorted()`, and `O(n)` otherwise.
    ///
    /// Reminder: duplicated `RowId`s results in undefined behavior.
    pub fn cell(
        &self,
        row_id: RowId,
        component_desc: &ComponentDescriptor,
    ) -> Option<ArrowArrayRef> {
        let list_array = self.components.get(component_desc)?;

        if self.is_sorted() {
            let row_ids = self.row_ids_slice();
            let index = row_ids.binary_search(&row_id).ok()?;
            list_array.is_valid(index).then(|| list_array.value(index))
        } else {
            self.row_ids()
                .find_position(|id| *id == row_id)
                .and_then(|(index, _)| list_array.is_valid(index).then(|| list_array.value(index)))
        }
    }

    /// Slices the [`Chunk`] vertically.
    ///
    /// The result is a new [`Chunk`] with the same columns and (potentially) less rows.
    ///
    /// This cannot fail nor panic: `index` and `len` will be capped so that they cannot
    /// run out of bounds.
    /// This can result in an empty [`Chunk`] being returned if the slice is completely OOB.
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn row_sliced(&self, index: usize, len: usize) -> Self {
        re_tracing::profile_function!();

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
            row_ids: row_ids.clone().slice(index, len),
            timelines: timelines
                .iter()
                .map(|(timeline, time_column)| (*timeline, time_column.row_sliced(index, len)))
                .collect(),
            components: components
                .iter()
                .map(|(component_desc, list_array)| {
                    (component_desc.clone(), list_array.clone().slice(index, len))
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
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn timeline_sliced(&self, timeline: TimelineName) -> Self {
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
                .map(|(timeline, time_column)| (*timeline, time_column.clone()))
                .into_iter()
                .collect(),
            components: components.clone(),
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Slices the [`Chunk`] horizontally by keeping only the selected `component_descr`.
    ///
    /// The result is a new [`Chunk`] with the same rows and (at-most) one component column.
    /// All non-component columns will be kept as-is.
    ///
    /// If `component_descr` is not found within the [`Chunk`], the end result will be the same as the
    /// current chunk but without any component column.
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn component_sliced(&self, component_descr: &ComponentDescriptor) -> Self {
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
            components: crate::ChunkComponents(
                components
                    .get(component_descr)
                    .map(|list_array| (component_descr.clone(), list_array.clone()))
                    .into_iter()
                    .collect(),
            ),
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
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn timelines_sliced(&self, timelines_to_keep: &IntSet<TimelineName>) -> Self {
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
                .map(|(timeline, time_column)| (*timeline, time_column.clone()))
                .collect(),
            components: components.clone(),
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Densifies the [`Chunk`] vertically based on the `component_descriptor` column.
    ///
    /// Densifying here means dropping all rows where the associated value in the `component_descriptor`
    /// column is null.
    ///
    /// The result is a new [`Chunk`] where the `component_descriptor` column is guaranteed to be dense.
    ///
    /// If `component_descriptor` doesn't exist in this [`Chunk`], or if it is already dense, this method
    /// is a no-op.
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn densified(&self, component_descr_pov: &ComponentDescriptor) -> Self {
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

        let Some(component_list_array) = self.components.get(component_descr_pov) else {
            return self.clone();
        };

        let Some(validity) = component_list_array.nulls() else {
            return self.clone();
        };

        re_tracing::profile_function!();

        let mask = validity.iter().collect_vec();
        let is_sorted = *is_sorted || (mask.iter().filter(|&&b| b).count() < 2);
        let validity_filter = ArrowBooleanArray::from(mask);

        let mut chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted,
            row_ids: re_arrow_util::filter_array(row_ids, &validity_filter),
            timelines: timelines
                .iter()
                .map(|(&timeline, time_column)| (timeline, time_column.filtered(&validity_filter)))
                .collect(),
            components: components
                .iter()
                .map(|(component_desc, list_array)| {
                    let filtered = re_arrow_util::filter_array(list_array, &validity_filter);
                    let filtered = if component_desc == component_descr_pov {
                        // Make sure we fully remove the validity bitmap for the densified
                        // component.
                        // This will allow further operations on this densified chunk to take some
                        // very optimized paths.
                        let (field, offsets, values, _nulls) = filtered.into_parts();
                        ArrowListArray::new(field, offsets, values, None)
                    } else {
                        filtered
                    };

                    (component_desc.clone(), filtered)
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
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn emptied(&self) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted: _,
            row_ids: _,
            timelines,
            components,
        } = self;

        re_tracing::profile_function!();

        Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted: true,
            row_ids: RowId::arrow_from_slice(&[]),
            timelines: timelines
                .iter()
                .map(|(&timeline, time_column)| (timeline, time_column.emptied()))
                .collect(),
            components: components
                .iter()
                .map(|(component_desc, list_array)| {
                    let field = match list_array.data_type() {
                        arrow::datatypes::DataType::List(field) => field.clone(),
                        _ => unreachable!("This is always s list array"),
                    };
                    (component_desc.clone(), ArrowListArray::new_null(field, 0))
                })
                .collect(),
        }
    }

    /// Removes all component columns from the [`Chunk`].
    ///
    /// The result is a new [`Chunk`] with the same number of rows and the same index columns, but
    /// no components.
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn components_removed(self) -> Self {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components: _,
        } = self;

        Self {
            id,
            entity_path,
            heap_size_bytes: Default::default(), // (!) lazily recompute
            is_sorted,
            row_ids,
            timelines,
            components: Default::default(),
        }
    }

    /// Removes duplicate rows from sections of consecutive identical indices.
    ///
    /// * If the [`Chunk`] is sorted on that index, the remaining values in the index column will be unique.
    /// * If the [`Chunk`] has been densified on a specific column, the resulting chunk will
    ///   effectively contain the latest value of that column for each given index value.
    ///
    /// If this is a temporal chunk and `timeline` isn't present in it, this method is a no-op.
    ///
    /// This does _not_ obey `RowId`-ordering semantics (or any other kind of semantics for that
    /// matter) -- it merely respects how the chunk is currently laid out: no more, no less.
    /// Sort the chunk according to the semantics you're looking for before calling this method.
    //
    // TODO(cmc): `Timeline` should really be `Index`.
    #[inline]
    pub fn deduped_latest_on_index(&self, index: &TimelineName) -> Self {
        re_tracing::profile_function!();

        if self.is_empty() {
            return self.clone();
        }

        if self.is_static() {
            return self.row_sliced(self.num_rows().saturating_sub(1), 1);
        }

        let Some(time_column) = self.timelines.get(index) else {
            return self.clone();
        };

        let indices = {
            let mut i = 0;
            let indices = time_column
                .times_raw()
                .iter()
                .copied()
                .dedup_with_count()
                .map(|(count, _time)| {
                    i += count;
                    i.saturating_sub(1) as i32
                })
                .collect_vec();
            arrow::array::Int32Array::from(indices)
        };

        let chunk = Self {
            id: self.id,
            entity_path: self.entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted: self.is_sorted,
            row_ids: re_arrow_util::take_array(
                &self.row_ids,
                &arrow::array::Int32Array::from(indices.clone()),
            ),
            timelines: self
                .timelines
                .iter()
                .map(|(&timeline, time_column)| (timeline, time_column.taken(&indices)))
                .collect(),
            components: self
                .components
                .iter()
                .map(|(component_desc, list_array)| {
                    let filtered = re_arrow_util::take_array(list_array, &indices);
                    (component_desc.clone(), filtered)
                })
                .collect(),
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)] // debug-only
        {
            chunk.sanity_check().unwrap();
        }

        chunk
    }

    /// Applies a [filter] kernel to the [`Chunk`] as a whole.
    ///
    /// Returns `None` if the length of the filter does not match the number of rows in the chunk.
    ///
    /// In release builds, filters are allowed to have null entries (they will be interpreted as `false`).
    /// In debug builds, null entries will panic.
    ///
    /// Note: a `filter` kernel _copies_ the data in order to make the resulting arrays contiguous in memory.
    ///
    /// [filter]: arrow::compute::kernels::filter
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn filtered(&self, filter: &ArrowBooleanArray) -> Option<Self> {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        // Safe early out to prevent panics in upstream kernel implementations.
        if filter.len() != self.num_rows() {
            return None;
        }

        if self.is_empty() {
            return Some(self.clone());
        }

        let num_filtered = filter.values().iter().filter(|&b| b).count();
        if num_filtered == 0 {
            return Some(self.emptied());
        }

        re_tracing::profile_function!();

        let is_sorted = *is_sorted || num_filtered < 2;

        let mut chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted,
            row_ids: re_arrow_util::filter_array(row_ids, filter),
            timelines: timelines
                .iter()
                .map(|(&timeline, time_column)| (timeline, time_column.filtered(filter)))
                .collect(),
            components: components
                .iter()
                .map(|(component_desc, list_array)| {
                    let filtered = re_arrow_util::filter_array(list_array, filter);
                    (component_desc.clone(), filtered)
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

        Some(chunk)
    }

    /// Applies a [take] kernel to the [`Chunk`] as a whole.
    ///
    /// In release builds, indices are allowed to have null entries (they will be taken as `null`s).
    /// In debug builds, null entries will panic.
    ///
    /// Note: a `take` kernel _copies_ the data in order to make the resulting arrays contiguous in memory.
    ///
    /// Takes care of up- and down-casting the data back and forth on behalf of the caller.
    ///
    /// [take]: arrow::compute::kernels::take
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn taken(&self, indices: &arrow::array::Int32Array) -> Self {
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

        if indices.is_empty() {
            return self.emptied();
        }

        re_tracing::profile_function!();

        let is_sorted = *is_sorted || (indices.len() < 2);

        let mut chunk = Self {
            id: *id,
            entity_path: entity_path.clone(),
            heap_size_bytes: Default::default(),
            is_sorted,
            row_ids: re_arrow_util::take_array(
                row_ids,
                &arrow::array::Int32Array::from(indices.clone()),
            ),
            timelines: timelines
                .iter()
                .map(|(&timeline, time_column)| (timeline, time_column.taken(indices)))
                .collect(),
            components: components
                .iter()
                .map(|(component_desc, list_array)| {
                    let taken = re_arrow_util::take_array(list_array, indices);
                    (component_desc.clone(), taken)
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
}

impl TimeColumn {
    /// Slices the [`TimeColumn`] vertically.
    ///
    /// The result is a new [`TimeColumn`] with the same timelines and (potentially) less rows.
    ///
    /// This cannot fail nor panic: `index` and `len` will be capped so that they cannot
    /// run out of bounds.
    /// This can result in an empty [`TimeColumn`] being returned if the slice is completely OOB.
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

        Self::new(is_sorted_opt, *timeline, times.clone().slice(index, len))
    }

    /// Empties the [`TimeColumn`] vertically.
    ///
    /// The result is a new [`TimeColumn`] with the same columns but zero rows.
    #[inline]
    pub fn emptied(&self) -> Self {
        let Self {
            timeline,
            times: _,
            is_sorted: _,
            time_range: _,
        } = self;

        Self::new(Some(true), *timeline, vec![].into())
    }

    /// Runs a [filter] compute kernel on the time data with the specified `mask`.
    ///
    /// [filter]: arrow::compute::kernels::filter
    #[inline]
    pub(crate) fn filtered(&self, filter: &ArrowBooleanArray) -> Self {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range: _,
        } = self;

        let is_sorted = *is_sorted || filter.values().iter().filter(|&b| b).count() < 2;

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
            re_arrow_util::filter_array(
                &arrow::array::Int64Array::new(times.clone(), None),
                filter,
            )
            .into_parts()
            .1,
        )
    }

    /// Runs a [take] compute kernel on the time data with the specified `indices`.
    ///
    /// [take]: arrow::compute::take
    #[inline]
    pub(crate) fn taken(&self, indices: &arrow::array::Int32Array) -> Self {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range: _,
        } = self;

        let new_times = re_arrow_util::take_array(
            &arrow::array::Int64Array::new(times.clone(), None),
            &arrow::array::Int32Array::from(indices.clone()),
        )
        .into_parts()
        .1;

        Self::new(Some(*is_sorted), *timeline, new_times)
    }
}

// ---

#[cfg(test)]
mod tests {
    use itertools::Itertools as _;
    use re_log_types::{
        TimePoint,
        example_components::{MyColor, MyLabel, MyPoint, MyPoints},
    };

    use crate::{Chunk, RowId, Timeline};

    use super::*;

    #[test]
    fn cell() -> anyhow::Result<()> {
        let entity_path = "my/entity";

        let row_id1 = RowId::ZERO.incremented_by(10);
        let row_id2 = RowId::ZERO.incremented_by(20);
        let row_id3 = RowId::ZERO.incremented_by(30);
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 1000),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 1032),
            (Timeline::new_sequence("frame"), 3),
        ];
        let timepoint3 = [
            (Timeline::log_time(), 1064),
            (Timeline::new_sequence("frame"), 5),
        ];
        let timepoint4 = [
            (Timeline::log_time(), 1096),
            (Timeline::new_sequence("frame"), 7),
        ];
        let timepoint5 = [
            (Timeline::log_time(), 1128),
            (Timeline::new_sequence("frame"), 9),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(6.0, 7.0)];

        let colors4 = &[MyColor::from_rgb(1, 1, 1)];
        let colors5 = &[MyColor::from_rgb(2, 2, 2), MyColor::from_rgb(3, 3, 3)];

        let labels1 = &[MyLabel("a".into())];
        let labels2 = &[MyLabel("b".into())];
        let labels3 = &[MyLabel("c".into())];
        let labels4 = &[MyLabel("d".into())];
        let labels5 = &[MyLabel("e".into())];

        let mut chunk = Chunk::builder(entity_path.into())
            .with_sparse_component_batches(
                row_id2,
                timepoint4,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors4 as _)),
                    (MyPoints::descriptor_labels(), Some(labels4 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id5,
                timepoint5,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors5 as _)),
                    (MyPoints::descriptor_labels(), Some(labels5 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id1,
                timepoint3,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels1 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id4,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id3,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels3 as _)),
                ],
            )
            .build()?;

        eprintln!("chunk:\n{chunk}");

        let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
            (row_id1, MyPoints::descriptor_points(), Some(points1 as _)),
            (row_id2, MyPoints::descriptor_labels(), Some(labels4 as _)),
            (row_id3, MyPoints::descriptor_colors(), None),
            (row_id4, MyPoints::descriptor_labels(), Some(labels2 as _)),
            (row_id5, MyPoints::descriptor_colors(), Some(colors5 as _)),
        ];

        assert!(!chunk.is_sorted());
        for (row_id, component_desc, expected) in expectations {
            let expected = expected
                .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
            eprintln!("{component_desc} @ {row_id}");
            similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_desc));
        }

        chunk.sort_if_unsorted();
        assert!(chunk.is_sorted());

        for (row_id, component_desc, expected) in expectations {
            let expected = expected
                .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
            eprintln!("{component_desc} @ {row_id}");
            similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_desc));
        }

        Ok(())
    }

    #[test]
    fn dedupe_temporal() -> anyhow::Result<()> {
        let entity_path = "my/entity";

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 1000),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 1032),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint3 = [
            (Timeline::log_time(), 1064),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint4 = [
            (Timeline::log_time(), 1096),
            (Timeline::new_sequence("frame"), 2),
        ];
        let timepoint5 = [
            (Timeline::log_time(), 1128),
            (Timeline::new_sequence("frame"), 2),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(6.0, 7.0)];

        let colors4 = &[MyColor::from_rgb(1, 1, 1)];
        let colors5 = &[MyColor::from_rgb(2, 2, 2), MyColor::from_rgb(3, 3, 3)];

        let labels1 = &[MyLabel("a".into())];
        let labels2 = &[MyLabel("b".into())];
        let labels3 = &[MyLabel("c".into())];
        let labels4 = &[MyLabel("d".into())];
        let labels5 = &[MyLabel("e".into())];

        let chunk = Chunk::builder(entity_path.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels1 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id4,
                timepoint4,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors4 as _)),
                    (MyPoints::descriptor_labels(), Some(labels4 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id5,
                timepoint5,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors5 as _)),
                    (MyPoints::descriptor_labels(), Some(labels5 as _)),
                ],
            )
            .build()?;

        eprintln!("chunk:\n{chunk}");

        {
            let got = chunk.deduped_latest_on_index(&TimelineName::new("frame"));
            eprintln!("got:\n{got}");
            assert_eq!(2, got.num_rows());

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id3, MyPoints::descriptor_points(), Some(points3 as _)),
                (row_id3, MyPoints::descriptor_colors(), None),
                (row_id3, MyPoints::descriptor_labels(), Some(labels3 as _)),
                //
                (row_id5, MyPoints::descriptor_points(), None),
                (row_id5, MyPoints::descriptor_colors(), Some(colors5 as _)),
                (row_id5, MyPoints::descriptor_labels(), Some(labels5 as _)),
            ];

            for (row_id, component_desc, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component_desc} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_desc));
            }
        }

        {
            let got = chunk.deduped_latest_on_index(&TimelineName::log_time());
            eprintln!("got:\n{got}");
            assert_eq!(5, got.num_rows());

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id1, MyPoints::descriptor_points(), Some(points1 as _)),
                (row_id1, MyPoints::descriptor_colors(), None),
                (row_id1, MyPoints::descriptor_labels(), Some(labels1 as _)),
                (row_id2, MyPoints::descriptor_points(), None),
                (row_id2, MyPoints::descriptor_colors(), None),
                (row_id2, MyPoints::descriptor_labels(), Some(labels2 as _)),
                (row_id3, MyPoints::descriptor_points(), Some(points3 as _)),
                (row_id3, MyPoints::descriptor_colors(), None),
                (row_id3, MyPoints::descriptor_labels(), Some(labels3 as _)),
                (row_id4, MyPoints::descriptor_points(), None),
                (row_id4, MyPoints::descriptor_colors(), Some(colors4 as _)),
                (row_id4, MyPoints::descriptor_labels(), Some(labels4 as _)),
                (row_id5, MyPoints::descriptor_points(), None),
                (row_id5, MyPoints::descriptor_colors(), Some(colors5 as _)),
                (row_id5, MyPoints::descriptor_labels(), Some(labels5 as _)),
            ];

            for (row_id, component_desc, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component_desc} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_desc));
            }
        }

        Ok(())
    }

    #[test]
    fn dedupe_static() -> anyhow::Result<()> {
        let entity_path = "my/entity";

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint_static = TimePoint::default();

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(6.0, 7.0)];

        let colors4 = &[MyColor::from_rgb(1, 1, 1)];
        let colors5 = &[MyColor::from_rgb(2, 2, 2), MyColor::from_rgb(3, 3, 3)];

        let labels1 = &[MyLabel("a".into())];
        let labels2 = &[MyLabel("b".into())];
        let labels3 = &[MyLabel("c".into())];
        let labels4 = &[MyLabel("d".into())];
        let labels5 = &[MyLabel("e".into())];

        let chunk = Chunk::builder(entity_path.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint_static.clone(),
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels1 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id2,
                timepoint_static.clone(),
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id3,
                timepoint_static.clone(),
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id4,
                timepoint_static.clone(),
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors4 as _)),
                    (MyPoints::descriptor_labels(), Some(labels4 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id5,
                timepoint_static.clone(),
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors5 as _)),
                    (MyPoints::descriptor_labels(), Some(labels5 as _)),
                ],
            )
            .build()?;

        eprintln!("chunk:\n{chunk}");

        {
            let got = chunk.deduped_latest_on_index(&TimelineName::new("frame"));
            eprintln!("got:\n{got}");
            assert_eq!(1, got.num_rows());

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id5, MyPoints::descriptor_points(), None),
                (row_id5, MyPoints::descriptor_colors(), Some(colors5 as _)),
                (row_id5, MyPoints::descriptor_labels(), Some(labels5 as _)),
            ];

            for (row_id, component_name, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component_name} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_name));
            }
        }

        {
            let got = chunk.deduped_latest_on_index(&TimelineName::log_time());
            eprintln!("got:\n{got}");
            assert_eq!(1, got.num_rows());

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id5, MyPoints::descriptor_points(), None),
                (row_id5, MyPoints::descriptor_colors(), Some(colors5 as _)),
                (row_id5, MyPoints::descriptor_labels(), Some(labels5 as _)),
            ];

            for (row_id, component_name, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component_name} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_name));
            }
        }

        Ok(())
    }

    #[test]
    fn filtered() -> anyhow::Result<()> {
        let entity_path = "my/entity";

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 1000),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 1032),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint3 = [
            (Timeline::log_time(), 1064),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint4 = [
            (Timeline::log_time(), 1096),
            (Timeline::new_sequence("frame"), 2),
        ];
        let timepoint5 = [
            (Timeline::log_time(), 1128),
            (Timeline::new_sequence("frame"), 2),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(6.0, 7.0)];

        let colors4 = &[MyColor::from_rgb(1, 1, 1)];
        let colors5 = &[MyColor::from_rgb(2, 2, 2), MyColor::from_rgb(3, 3, 3)];

        let labels1 = &[MyLabel("a".into())];
        let labels2 = &[MyLabel("b".into())];
        let labels3 = &[MyLabel("c".into())];
        let labels4 = &[MyLabel("d".into())];
        let labels5 = &[MyLabel("e".into())];

        let chunk = Chunk::builder(entity_path.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels1 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id4,
                timepoint4,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors4 as _)),
                    (MyPoints::descriptor_labels(), Some(labels4 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id5,
                timepoint5,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors5 as _)),
                    (MyPoints::descriptor_labels(), Some(labels5 as _)),
                ],
            )
            .build()?;

        eprintln!("chunk:\n{chunk}");

        // basic
        {
            let filter =
                ArrowBooleanArray::from((0..chunk.num_rows()).map(|i| i % 2 == 0).collect_vec());
            let got = chunk.filtered(&filter).unwrap();
            eprintln!("got:\n{got}");
            assert_eq!(
                filter.values().iter().filter(|&b| b).count(),
                got.num_rows()
            );

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id1, MyPoints::descriptor_points(), Some(points1 as _)),
                (row_id1, MyPoints::descriptor_colors(), None),
                (row_id1, MyPoints::descriptor_labels(), Some(labels1 as _)),
                //
                (row_id3, MyPoints::descriptor_points(), Some(points3 as _)),
                (row_id3, MyPoints::descriptor_colors(), None),
                (row_id3, MyPoints::descriptor_labels(), Some(labels3 as _)),
                //
                (row_id5, MyPoints::descriptor_points(), None),
                (row_id5, MyPoints::descriptor_colors(), Some(colors5 as _)),
                (row_id5, MyPoints::descriptor_labels(), Some(labels5 as _)),
            ];

            for (row_id, component_name, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component_name} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_name));
            }
        }

        // shorter
        {
            let filter = ArrowBooleanArray::from(
                (0..chunk.num_rows() / 2).map(|i| i % 2 == 0).collect_vec(),
            );
            let got = chunk.filtered(&filter);
            assert!(got.is_none());
        }

        // longer
        {
            let filter = ArrowBooleanArray::from(
                (0..chunk.num_rows() * 2).map(|i| i % 2 == 0).collect_vec(),
            );
            let got = chunk.filtered(&filter);
            assert!(got.is_none());
        }

        Ok(())
    }

    #[test]
    fn taken() -> anyhow::Result<()> {
        use arrow::array::Int32Array as ArrowInt32Array;

        let entity_path = "my/entity";

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timepoint1 = [
            (Timeline::log_time(), 1000),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint2 = [
            (Timeline::log_time(), 1032),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint3 = [
            (Timeline::log_time(), 1064),
            (Timeline::new_sequence("frame"), 1),
        ];
        let timepoint4 = [
            (Timeline::log_time(), 1096),
            (Timeline::new_sequence("frame"), 2),
        ];
        let timepoint5 = [
            (Timeline::log_time(), 1128),
            (Timeline::new_sequence("frame"), 2),
        ];

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(6.0, 7.0)];

        let colors4 = &[MyColor::from_rgb(1, 1, 1)];
        let colors5 = &[MyColor::from_rgb(2, 2, 2), MyColor::from_rgb(3, 3, 3)];

        let labels1 = &[MyLabel("a".into())];
        let labels2 = &[MyLabel("b".into())];
        let labels3 = &[MyLabel("c".into())];
        let labels4 = &[MyLabel("d".into())];
        let labels5 = &[MyLabel("e".into())];

        let chunk = Chunk::builder(entity_path.into())
            .with_sparse_component_batches(
                row_id1,
                timepoint1,
                [
                    (MyPoints::descriptor_points(), Some(points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels1 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id2,
                timepoint2,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels2 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id3,
                timepoint3,
                [
                    (MyPoints::descriptor_points(), Some(points3 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(labels3 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id4,
                timepoint4,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors4 as _)),
                    (MyPoints::descriptor_labels(), Some(labels4 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id5,
                timepoint5,
                [
                    (MyPoints::descriptor_points(), None),
                    (MyPoints::descriptor_colors(), Some(colors5 as _)),
                    (MyPoints::descriptor_labels(), Some(labels5 as _)),
                ],
            )
            .build()?;

        eprintln!("chunk:\n{chunk}");

        // basic
        {
            let indices = ArrowInt32Array::from(
                (0..chunk.num_rows() as i32)
                    .filter(|i| i % 2 == 0)
                    .collect_vec(),
            );
            let got = chunk.taken(&indices);
            eprintln!("got:\n{got}");
            assert_eq!(indices.len(), got.num_rows());

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id1, MyPoints::descriptor_points(), Some(points1 as _)),
                (row_id1, MyPoints::descriptor_colors(), None),
                (row_id1, MyPoints::descriptor_labels(), Some(labels1 as _)),
                //
                (row_id3, MyPoints::descriptor_points(), Some(points3 as _)),
                (row_id3, MyPoints::descriptor_colors(), None),
                (row_id3, MyPoints::descriptor_labels(), Some(labels3 as _)),
                //
                (row_id5, MyPoints::descriptor_points(), None),
                (row_id5, MyPoints::descriptor_colors(), Some(colors5 as _)),
                (row_id5, MyPoints::descriptor_labels(), Some(labels5 as _)),
            ];

            for (row_id, component_name, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component_name} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_name));
            }
        }

        // repeated
        {
            let indices = ArrowInt32Array::from(
                std::iter::repeat(2i32)
                    .take(chunk.num_rows() * 2)
                    .collect_vec(),
            );
            let got = chunk.taken(&indices);
            eprintln!("got:\n{got}");
            assert_eq!(indices.len(), got.num_rows());
        }

        Ok(())
    }
}
