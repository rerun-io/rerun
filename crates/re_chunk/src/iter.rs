use arrow2::array::Array as ArrowArray;

use re_log_types::{TimeInt, Timeline};
use re_types_core::ComponentName;

use crate::{Chunk, RowId};

// ---

impl Chunk {
    /// Returns an iterator over the rows of the [`Chunk`].
    ///
    /// Each yielded item is a component batch with its associated index ([`RowId`] + data time).
    ///
    /// Iterating a [`Chunk`] on a row basis is very wasteful, performance-wise.
    /// Prefer columnar access when possible.
    //
    // TODO(cmc): a row-based iterator is obviously not what we want -- one of the benefits of
    // chunks is to amortize the cost of downcasting & "deserialization".
    // But at the moment we still need to run with the native deserialization cache, which expects
    // row-based data.
    // As soon as we remove the native cache and start exposing `Chunk`s directly to downstream
    // systems, we will look into ergonomic ways to do columnar access.
    pub fn iter_rows(
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
            .flat_map(|time_chunk| time_chunk.times().collect::<Vec<_>>())
            // If there's no time data, then the associate data time must be `TimeInt::STATIC`.
            .chain(std::iter::repeat(TimeInt::STATIC));

        let arrays = components
            .get(component_name)
            .into_iter()
            .flat_map(|list_array| list_array.into_iter());

        itertools::izip!(data_times, row_ids, arrays)
    }
}
