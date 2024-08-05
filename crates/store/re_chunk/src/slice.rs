use arrow2::array::{
    Array as ArrowArray, BooleanArray as ArrowBooleanArray, ListArray,
    PrimitiveArray as ArrowPrimitiveArray, StructArray,
};

use itertools::Itertools;
use nohash_hasher::IntSet;

use re_log_types::Timeline;
use re_types_core::ComponentName;

use crate::{Chunk, ChunkTimeline, RowId};

// ---

// NOTE: Not worth writing tests for all of these, until some subtle bug comes up.
// Most of them are indirectly stressed by our higher-level query tests anyhow.

impl Chunk {
    /// Returns the cell corresponding to the specified [`RowId`] for a given [`ComponentName`].
    ///
    /// This is `O(log(n))` if `self.is_sorted()`, and `O(n)` otherwise.
    ///
    /// Reminder: duplicated `RowId`s results in undefined behavior.
    pub fn cell(
        &self,
        row_id: RowId,
        component_name: &ComponentName,
    ) -> Option<Box<dyn ArrowArray>> {
        let list_array = self.components.get(component_name)?;

        if self.is_sorted() {
            let row_id_128 = row_id.as_u128();
            let row_id_time_ns = (row_id_128 >> 64) as u64;
            let row_id_inc = (row_id_128 & (!0 >> 64)) as u64;

            let (times, incs) = self.row_ids_raw();
            let times = times.values().as_slice();
            let incs = incs.values().as_slice();

            let mut index = times.partition_point(|&time| time < row_id_time_ns);
            while index < incs.len() && incs[index] < row_id_inc {
                index += 1;
            }

            let found_it =
                times.get(index) == Some(&row_id_time_ns) && incs.get(index) == Some(&row_id_inc);

            (found_it && list_array.is_valid(index)).then(|| list_array.value(index))
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
    pub fn densified(&self, component_name_pov: ComponentName) -> Self {
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

        let Some(component_list_array) = components.get(&component_name_pov) else {
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
                    let filtered = crate::util::filter_array(list_array, &validity_filter);
                    let filtered = if component_name == component_name_pov {
                        // Make sure we fully remove the validity bitmap for the densified
                        // component.
                        // This will allow further operations on this densified chunk to take some
                        // very optimized paths.

                        #[allow(clippy::unwrap_used)]
                        filtered
                            .with_validity(None)
                            .as_any()
                            .downcast_ref::<ListArray<i32>>()
                            // Unwrap: cannot possibly fail -- going from a ListArray back to a ListArray.
                            .unwrap()
                            .clone()
                    } else {
                        filtered
                    };

                    (component_name, filtered)
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

// ---

#[cfg(test)]
mod tests {
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint};
    use re_types_core::{ComponentBatch, Loggable};

    use crate::{Chunk, RowId, Timeline};

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
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors4 as _)),
                    (MyLabel::name(), Some(labels4 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id5,
                timepoint5,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), Some(colors5 as _)),
                    (MyLabel::name(), Some(labels5 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id1,
                timepoint3,
                [
                    (MyPoint::name(), Some(points1 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), Some(labels1 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id4,
                timepoint2,
                [
                    (MyPoint::name(), None),
                    (MyColor::name(), None),
                    (MyLabel::name(), Some(labels2 as _)),
                ],
            )
            .with_sparse_component_batches(
                row_id3,
                timepoint1,
                [
                    (MyPoint::name(), Some(points3 as _)),
                    (MyColor::name(), None),
                    (MyLabel::name(), Some(labels3 as _)),
                ],
            )
            .build()?;

        eprintln!("chunk:\n{chunk}");

        let expectations: &[(_, _, Option<&dyn ComponentBatch>)] = &[
            (row_id1, MyPoint::name(), Some(points1 as _)),
            (row_id2, MyLabel::name(), Some(labels4 as _)),
            (row_id3, MyColor::name(), None),
            (row_id4, MyLabel::name(), Some(labels2 as _)),
            (row_id5, MyColor::name(), Some(colors5 as _)),
        ];

        assert!(!chunk.is_sorted());
        for (row_id, component_name, expected) in expectations {
            let expected =
                expected.and_then(|expected| re_types_core::LoggableBatch::to_arrow(expected).ok());
            eprintln!("{component_name} @ {row_id}");
            similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_name));
        }

        chunk.sort_if_unsorted();
        assert!(chunk.is_sorted());

        for (row_id, component_name, expected) in expectations {
            let expected =
                expected.and_then(|expected| re_types_core::LoggableBatch::to_arrow(expected).ok());
            eprintln!("{component_name} @ {row_id}");
            similar_asserts::assert_eq!(expected, chunk.cell(*row_id, component_name));
        }

        Ok(())
    }
}
