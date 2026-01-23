use arrow::array::{
    Array as _, ArrayRef as ArrowArrayRef, BooleanArray as ArrowBooleanArray,
    ListArray as ArrowListArray,
};
use itertools::Itertools as _;
use nohash_hasher::IntSet;
use re_log_types::TimelineName;
use re_types_core::{ComponentIdentifier, SerializedComponentColumn};

use crate::{Chunk, RowId, TimeColumn};

// ---

// NOTE: Not worth writing tests for all of these, until some subtle bug comes up.
// Most of them are indirectly stressed by our higher-level query tests anyhow.

impl Chunk {
    /// Returns the cell corresponding to the specified [`RowId`] for a given [`re_types_core::ComponentIdentifier`].
    ///
    /// This is `O(log(n))` if `self.is_sorted()`, and `O(n)` otherwise.
    ///
    /// Reminder: duplicated `RowId`s results in undefined behavior.
    pub fn cell(&self, row_id: RowId, component: ComponentIdentifier) -> Option<ArrowArrayRef> {
        let list_array = self.components.get_array(component)?;

        if self.is_sorted() {
            let row_ids = self.row_ids_slice();
            let index = row_ids.binary_search(&row_id).ok()?;
            list_array.is_valid(index).then(|| list_array.value(index))
        } else {
            let (index, _) = self.row_ids().find_position(|id| *id == row_id)?;
            list_array.is_valid(index).then(|| list_array.value(index))
        }
    }

    /// Shallow-slices the [`Chunk`] vertically.
    ///
    /// The result is a new [`Chunk`] with the same columns and (potentially) less rows.
    ///
    /// This cannot fail nor panic: `index` and `len` will be capped so that they cannot
    /// run out of bounds.
    /// This can result in an empty [`Chunk`] being returned if the slice is completely OOB.
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    ///
    /// ## When to use shallow vs. deep slicing?
    ///
    /// This operation is shallow and therefore always O(1), which implicitly means that it cannot
    /// ever modify the values of the offsets themselves.
    /// Since the offsets are left untouched, the original unsliced data must always be kept around
    /// too, _even if the sliced data were to be written to disk_.
    /// Similarly, the byte sizes reported by e.g. `Chunk::heap_size_bytes` might not always make intuitive
    /// sense, and should be used very carefully.
    ///
    /// For these reasons, shallow slicing should only be used in the context of short-term, in-memory storage
    /// (e.g. when slicing the results of a query).
    /// When slicing data for long-term storage, whether in-memory or on disk, see [`Self::row_sliced_deep`] instead.
    #[must_use]
    pub fn row_sliced_shallow(&self, index: usize, len: usize) -> Self {
        let deep = false;
        self.row_sliced_impl(index, len, deep)
    }

    /// Deep-slices the [`Chunk`] vertically.
    ///
    /// The result is a new [`Chunk`] with the same columns and (potentially) less rows.
    ///
    /// This cannot fail nor panic: `index` and `len` will be capped so that they cannot
    /// run out of bounds.
    /// This can result in an empty [`Chunk`] being returned if the slice is completely OOB.
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    ///
    /// ## When to use shallow vs. deep slicing?
    ///
    /// This operation is deep and therefore always O(N).
    ///
    /// The underlying data, offsets, bitmaps and other buffers required will be reallocated, copied around,
    /// and patched as much as required so that the resulting physical data becomes as packed as possible for
    /// the desired slice.
    /// Similarly, the byte sizes reported by e.g. `Chunk::heap_size_bytes` should always match intuitive expectations.
    ///
    /// These characteristics make deep slicing very useful for longer term data, whether it's stored
    /// in-memory (e.g. in a `ChunkStore`), or on disk.
    /// When slicing data for short-term needs (e.g. slicing the results of a query) prefer [`Self::row_sliced_shallow`] instead.
    #[must_use]
    pub fn row_sliced_deep(&self, index: usize, len: usize) -> Self {
        let deep = true;
        self.row_sliced_impl(index, len, deep)
    }

    #[must_use]
    fn row_sliced_impl(&self, index: usize, len: usize, deep: bool) -> Self {
        re_tracing::profile_function!(if deep { "deep" } else { "shallow" });

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
            row_ids: if deep {
                re_arrow_util::deep_slice_array(row_ids, index, len)
            } else {
                row_ids.slice(index, len)
            },
            timelines: timelines
                .iter()
                .map(|(timeline, time_column)| (*timeline, time_column.row_sliced(index, len)))
                .collect(),
            components: components
                .values()
                .map(|column| {
                    SerializedComponentColumn::new(
                        if deep {
                            re_arrow_util::deep_slice_array(&column.list_array, index, len)
                        } else {
                            column.list_array.slice(index, len)
                        },
                        column.descriptor.clone(),
                    )
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
        #[expect(clippy::unwrap_used)] // debug-only
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
        #[expect(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Slices the [`Chunk`] horizontally by keeping only the selected `component`.
    ///
    /// The result is a new [`Chunk`] with the same rows and (at-most) one component column.
    /// All non-component columns will be kept as-is.
    ///
    /// If `component` is not found within the [`Chunk`], the end result will be the same as the
    /// current chunk but without any component column.
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn component_sliced(&self, component: ComponentIdentifier) -> Self {
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
                .get(component)
                .map(|column| {
                    SerializedComponentColumn::new(
                        column.list_array.clone(),
                        column.descriptor.clone(),
                    )
                })
                .into_iter()
                .collect(),
        };

        #[cfg(debug_assertions)]
        #[expect(clippy::unwrap_used)] // debug-only
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
        #[expect(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        chunk
    }

    /// Densifies the [`Chunk`] vertically based on the `component_pov` column.
    ///
    /// Densifying here means dropping all rows where the associated value in the `component_pov`
    /// column is null.
    ///
    /// The result is a new [`Chunk`] where the `component_pov` column is guaranteed to be dense.
    ///
    /// If `component_pov` doesn't exist in this [`Chunk`], or if it is already dense, this method
    /// is a no-op.
    ///
    /// Returns `false` if the operation was a no-op (i.e. the chunk was already dense), true otherwise
    /// (i.e. the data had to be reallocated).
    ///
    /// WARNING: the returned chunk has the same old [`crate::ChunkId`]! Change it with [`Self::with_id`].
    #[must_use]
    #[inline]
    pub fn densified(&self, component_pov: ComponentIdentifier) -> (Self, bool) {
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
            return (self.clone(), false);
        }

        let Some(component_list_array) = self.components.get_array(component_pov) else {
            return (self.clone(), false);
        };

        let Some(validity) = component_list_array.nulls() else {
            return (self.clone(), false);
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
                .values()
                .map(|column| {
                    let filtered =
                        re_arrow_util::filter_array(&column.list_array, &validity_filter);
                    let filtered = if column.descriptor.component == component_pov {
                        // Make sure we fully remove the validity bitmap for the densified
                        // component.
                        // This will allow further operations on this densified chunk to take some
                        // very optimized paths.
                        let (field, offsets, values, _nulls) = filtered.into_parts();
                        ArrowListArray::new(field, offsets, values, None)
                    } else {
                        filtered
                    };

                    SerializedComponentColumn::new(filtered, column.descriptor.clone())
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
        #[expect(clippy::unwrap_used)] // debug-only
        chunk.sanity_check().unwrap();

        (chunk, true)
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
                .values()
                .map(|column| {
                    let field = match column.list_array.data_type() {
                        arrow::datatypes::DataType::List(field) => field.clone(),
                        _ => unreachable!("This is always s list array"),
                    };
                    SerializedComponentColumn::new(
                        ArrowListArray::new_null(field, 0),
                        column.descriptor.clone(),
                    )
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
            return self.row_sliced_shallow(self.num_rows().saturating_sub(1), 1);
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

                    // input is a 32-bit array, so this can't overflow/wrap
                    #[expect(clippy::cast_possible_wrap)]
                    {
                        i.saturating_sub(1) as i32
                    }
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
                .values()
                .map(|column| {
                    let filtered = re_arrow_util::take_array(&column.list_array, &indices);
                    SerializedComponentColumn::new(filtered, column.descriptor.clone())
                })
                .collect(),
        };

        #[cfg(debug_assertions)]
        #[expect(clippy::unwrap_used)] // debug-only
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
                .values()
                .map(|column| {
                    let filtered = re_arrow_util::filter_array(&column.list_array, filter);
                    SerializedComponentColumn::new(filtered, column.descriptor.clone())
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
        #[expect(clippy::unwrap_used)] // debug-only
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
                .values()
                .map(|column| {
                    let taken = re_arrow_util::take_array(&column.list_array, indices);
                    SerializedComponentColumn::new(taken, column.descriptor.clone())
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
        #[expect(clippy::unwrap_used)] // debug-only
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
    #![expect(clippy::cast_possible_wrap)]

    use itertools::Itertools as _;
    use re_log_types::{
        TimePoint,
        example_components::{MyColor, MyLabel, MyPoint, MyPoints},
    };

    use super::*;
    use crate::{Chunk, RowId, Timeline};

    #[test]
    fn cell() -> anyhow::Result<()> {
        let mypoints_points_component = MyPoints::descriptor_points().component;
        let mypoints_colors_component = MyPoints::descriptor_colors().component;
        let mypoints_labels_component = MyPoints::descriptor_labels().component;

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

        let mut chunk = Chunk::builder(entity_path)
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
            (row_id1, mypoints_points_component, Some(points1 as _)),
            (row_id2, mypoints_labels_component, Some(labels4 as _)),
            (row_id3, mypoints_colors_component, None),
            (row_id4, mypoints_labels_component, Some(labels2 as _)),
            (row_id5, mypoints_colors_component, Some(colors5 as _)),
        ];

        assert!(!chunk.is_sorted());
        for (row_id, component, expected) in expectations {
            let expected = expected
                .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
            eprintln!("{component} @ {row_id}");
            similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
        }

        chunk.sort_if_unsorted();
        assert!(chunk.is_sorted());

        for (row_id, component, expected) in expectations {
            let expected = expected
                .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
            eprintln!("{component} @ {row_id}");
            similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
        }

        Ok(())
    }

    #[test]
    fn dedupe_temporal() -> anyhow::Result<()> {
        let mypoints_points_component = MyPoints::descriptor_points().component;
        let mypoints_colors_component = MyPoints::descriptor_colors().component;
        let mypoints_labels_component = MyPoints::descriptor_labels().component;

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

        let chunk = Chunk::builder(entity_path)
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
                (row_id3, mypoints_points_component, Some(points3 as _)),
                (row_id3, mypoints_colors_component, None),
                (row_id3, mypoints_labels_component, Some(labels3 as _)),
                //
                (row_id5, mypoints_points_component, None),
                (row_id5, mypoints_colors_component, Some(colors5 as _)),
                (row_id5, mypoints_labels_component, Some(labels5 as _)),
            ];

            for (row_id, component, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
            }
        }

        {
            let got = chunk.deduped_latest_on_index(&TimelineName::log_time());
            eprintln!("got:\n{got}");
            assert_eq!(5, got.num_rows());

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id1, mypoints_points_component, Some(points1 as _)),
                (row_id1, mypoints_colors_component, None),
                (row_id1, mypoints_labels_component, Some(labels1 as _)),
                (row_id2, mypoints_points_component, None),
                (row_id2, mypoints_colors_component, None),
                (row_id2, mypoints_labels_component, Some(labels2 as _)),
                (row_id3, mypoints_points_component, Some(points3 as _)),
                (row_id3, mypoints_colors_component, None),
                (row_id3, mypoints_labels_component, Some(labels3 as _)),
                (row_id4, mypoints_points_component, None),
                (row_id4, mypoints_colors_component, Some(colors4 as _)),
                (row_id4, mypoints_labels_component, Some(labels4 as _)),
                (row_id5, mypoints_points_component, None),
                (row_id5, mypoints_colors_component, Some(colors5 as _)),
                (row_id5, mypoints_labels_component, Some(labels5 as _)),
            ];

            for (row_id, component, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
            }
        }

        Ok(())
    }

    #[test]
    fn dedupe_static() -> anyhow::Result<()> {
        let mypoints_points_component = MyPoints::descriptor_points().component;
        let mypoints_colors_component = MyPoints::descriptor_colors().component;
        let mypoints_labels_component = MyPoints::descriptor_labels().component;

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

        let chunk = Chunk::builder(entity_path)
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
                (row_id5, mypoints_points_component, None),
                (row_id5, mypoints_colors_component, Some(colors5 as _)),
                (row_id5, mypoints_labels_component, Some(labels5 as _)),
            ];

            for (row_id, component, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
            }
        }

        {
            let got = chunk.deduped_latest_on_index(&TimelineName::log_time());
            eprintln!("got:\n{got}");
            assert_eq!(1, got.num_rows());

            let expectations: &[(_, _, Option<&dyn re_types_core::ComponentBatch>)] = &[
                (row_id5, mypoints_points_component, None),
                (row_id5, mypoints_colors_component, Some(colors5 as _)),
                (row_id5, mypoints_labels_component, Some(labels5 as _)),
            ];

            for (row_id, component, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
            }
        }

        Ok(())
    }

    #[test]
    fn filtered() -> anyhow::Result<()> {
        let mypoints_points_component = MyPoints::descriptor_points().component;
        let mypoints_colors_component = MyPoints::descriptor_colors().component;
        let mypoints_labels_component = MyPoints::descriptor_labels().component;

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

        let chunk = Chunk::builder(entity_path)
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
                (row_id1, mypoints_points_component, Some(points1 as _)),
                (row_id1, mypoints_colors_component, None),
                (row_id1, mypoints_labels_component, Some(labels1 as _)),
                //
                (row_id3, mypoints_points_component, Some(points3 as _)),
                (row_id3, mypoints_colors_component, None),
                (row_id3, mypoints_labels_component, Some(labels3 as _)),
                //
                (row_id5, mypoints_points_component, None),
                (row_id5, mypoints_colors_component, Some(colors5 as _)),
                (row_id5, mypoints_labels_component, Some(labels5 as _)),
            ];

            for (row_id, component, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
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

        let mypoints_points_component = MyPoints::descriptor_points().component;
        let mypoints_colors_component = MyPoints::descriptor_colors().component;
        let mypoints_labels_component = MyPoints::descriptor_labels().component;

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

        let chunk = Chunk::builder(entity_path)
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
                (row_id1, mypoints_points_component, Some(points1 as _)),
                (row_id1, mypoints_colors_component, None),
                (row_id1, mypoints_labels_component, Some(labels1 as _)),
                //
                (row_id3, mypoints_points_component, Some(points3 as _)),
                (row_id3, mypoints_colors_component, None),
                (row_id3, mypoints_labels_component, Some(labels3 as _)),
                //
                (row_id5, mypoints_points_component, None),
                (row_id5, mypoints_colors_component, Some(colors5 as _)),
                (row_id5, mypoints_labels_component, Some(labels5 as _)),
            ];

            for (row_id, component, expected) in expectations {
                let expected = expected
                    .and_then(|expected| re_types_core::ComponentBatch::to_arrow(expected).ok());
                eprintln!("{component} @ {row_id}");
                similar_asserts::assert_eq!(expected, chunk.cell(*row_id, *component));
            }
        }

        // repeated
        {
            let indices = ArrowInt32Array::from(
                std::iter::repeat_n(2i32, chunk.num_rows() * 2).collect_vec(),
            );
            let got = chunk.taken(&indices);
            eprintln!("got:\n{got}");
            assert_eq!(indices.len(), got.num_rows());
        }

        Ok(())
    }

    #[test]
    fn slice_memory_size_conservation() -> anyhow::Result<()> {
        use arrow::array::{ListArray as ArrowListArray, UInt8Array as ArrowUInt8Array};
        use arrow::buffer::OffsetBuffer as ArrowOffsetBuffer;
        use re_byte_size::SizeBytes as _;
        use re_types_core::{ComponentDescriptor, SerializedComponentColumn};

        // Create a chunk with 3 rows of raw blob data with different sizes
        let entity_path = "test/entity";

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();

        // Create blob data of different sizes
        let blob_size_1 = 10_000; // 10KB
        let blob_size_2 = 20_000; // 20KB
        let blob_size_3 = 30_000; // 30KB

        let blob_data_1: Vec<u8> = (0..blob_size_1 as u8).cycle().take(blob_size_1).collect();
        let blob_data_2: Vec<u8> = (0..blob_size_2 as u8).cycle().take(blob_size_2).collect();
        let blob_data_3: Vec<u8> = (0..blob_size_3 as u8).cycle().take(blob_size_3).collect();

        // Combine all blob data for the ListArray
        let mut all_blob_data: Vec<u8> = Vec::new();
        all_blob_data.extend(&blob_data_1);
        all_blob_data.extend(&blob_data_2);
        all_blob_data.extend(&blob_data_3);

        // Create the inner UInt8Array containing all blob data
        let values_array = ArrowUInt8Array::from(all_blob_data);

        // Create the ListArray
        let list_array = ArrowListArray::new(
            arrow::datatypes::Field::new("item", arrow::datatypes::DataType::UInt8, false).into(),
            ArrowOffsetBuffer::from_lengths([blob_size_1, blob_size_2, blob_size_3]),
            std::sync::Arc::new(values_array),
            None,
        );

        // Create component descriptor
        let blob_descriptor = ComponentDescriptor::partial("blob");

        // Create component column
        let component_column = SerializedComponentColumn::new(list_array, blob_descriptor);

        // Create the chunk manually with raw component data
        let chunk = Chunk::new(
            crate::ChunkId::new(),
            re_log_types::EntityPath::from(entity_path),
            Some(true), // is_sorted
            RowId::arrow_from_slice(&[row_id1, row_id2, row_id3]),
            std::iter::once((
                *Timeline::new_sequence("frame").name(),
                crate::TimeColumn::new_sequence("frame", [1, 2, 3]),
            ))
            .collect(),
            std::iter::once(component_column).collect(),
        )?;

        let original_size = chunk.heap_size_bytes();
        eprintln!("Original chunk size: {original_size} bytes");

        // Create 3 single-row slices
        let slice1 = chunk.row_sliced_deep(0, 1);
        let slice2 = chunk.row_sliced_deep(1, 1);
        let slice3 = chunk.row_sliced_deep(2, 1);

        let slice1_size = slice1.heap_size_bytes();
        let slice2_size = slice2.heap_size_bytes();
        let slice3_size = slice3.heap_size_bytes();

        eprintln!("Slice 1 size: {slice1_size} bytes ({blob_size_1} byte blob)");
        eprintln!("Slice 2 size: {slice2_size} bytes ({blob_size_2} byte blob)");
        eprintln!("Slice 3 size: {slice3_size} bytes ({blob_size_3} byte blob)");

        let total_slice_size = slice1_size + slice2_size + slice3_size;
        eprintln!("Total slices size: {total_slice_size} bytes");

        // The slices should add up to approximately the original size
        // We allow some overhead for metadata duplication (row IDs, timeline data, etc.)
        // but the component data should be accurately sliced
        let acceptable_overhead = 2600; // bytes for metadata overhead (increased for raw arrays)

        assert!(
            total_slice_size <= original_size + acceptable_overhead,
            "Slices total size ({total_slice_size}) should not exceed original size ({original_size}) by more than {acceptable_overhead} bytes of overhead",
        );

        // Each slice should be proportional to its data size
        // The slice with 30KB should be larger than the slice with 10KB
        assert!(
            slice3_size > slice1_size,
            "Slice 3 with {blob_size_3} bytes ({slice3_size} total bytes) should be larger than slice 1 with {blob_size_1} bytes ({slice1_size} total bytes)",
        );

        assert!(
            slice2_size > slice1_size,
            "Slice 2 with {blob_size_2} bytes ({slice2_size} total bytes) should be larger than slice 1 with {blob_size_1} bytes ({slice1_size} total bytes)",
        );

        // Verify that the sliced data actually reflects the expected blob sizes
        // The component data size should be roughly proportional to the blob sizes
        let size_ratio_3_to_1 = slice3_size as f64 / slice1_size as f64;
        let expected_ratio_3_to_1 = blob_size_3 as f64 / blob_size_1 as f64; // 3.0

        assert!(
            size_ratio_3_to_1 > 2.0 && size_ratio_3_to_1 < 4.0,
            "Size ratio between slice 3 and slice 1 ({size_ratio_3_to_1:.2}) should be close to expected blob ratio ({expected_ratio_3_to_1:.2})",
        );

        eprintln!("✓ Raw arrow array slice memory calculation test passed!");

        Ok(())
    }
}
