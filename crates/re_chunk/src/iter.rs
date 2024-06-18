use arrow2::array::Array as ArrowArray;

use itertools::izip;
use re_log_types::{TimeInt, Timeline};
use re_types_core::{Component, ComponentName};

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

// TODO: what we'll want is a way to range_indexed() but in a smart way...

impl Chunk {
    // TODO: implement the logic where empty outputs as None instead
    // TODO: so these really should be options then i guess

    pub fn iter_indexed<C: Component>(
        &self,
        timeline: &Timeline,
    ) -> impl Iterator<Item = ((TimeInt, RowId), Vec<C>)> + '_ {
        // TODO: let's imagine we're densified and sorted and everything -- what now?
        // TODO: what if we're static though
        // TODO: unless maybe we want both?

        let list_array = self.components.get(&C::name()).unwrap();
        let mut all = C::from_arrow(&**list_array.values()).unwrap();
        let splits = list_array.offsets().lengths().map(move |len| {
            // TODO: makes no sense whatsoever
            let new = all.split_off(len);
            let yielded = std::mem::take(&mut all);
            all = new;
            yielded
        });

        let mut i = 0;
        izip!(self.indices(timeline).unwrap(), splits).filter(move |(index, batch)| {
            let is_valid = list_array.is_valid(i);
            i += 1;
            is_valid
        })
    }

    pub fn iter_batches<C: Component>(&self) -> impl Iterator<Item = Vec<C>> + '_ {
        // TODO: let's imagine we're densified and sorted and everything -- what now?
        // TODO: what if we're static though

        // TODO: is this where we're supposed to map empty array to none?

        let list_array = self.components.get(&C::name()).unwrap();
        let mut all = C::from_arrow(&**list_array.values()).unwrap();
        let splits = list_array.offsets().lengths().map(move |len| {
            // TODO: makes no sense whatsoever
            let new = all.split_off(len);
            let yielded = std::mem::take(&mut all);
            all = new;
            yielded
        });

        let mut i = 0;
        splits.filter(move |batch| {
            let is_valid = list_array.is_valid(i);
            i += 1;
            is_valid
        })
    }

    // TODO: we cannot really implement this
    // pub fn iter_indexed_raw<A: ArrowArray>(
    //     &self,
    //     timeline: &Timeline,
    //     component_name: &ComponentName,
    // ) -> impl Iterator<Item = ((TimeInt, RowId), A)> + '_ {
    //     // TODO: let's imagine we're densified and sorted and everything -- what now?
    //     // TODO: what if we're static though
    //
    //     let list_array = self.components.get(component_name).unwrap();
    //     let array = list_array.values().as_any().downcast_ref::<A>().unwrap();
    //
    //     let splits = izip!(list_array.offsets().iter(), list_array.offsets().lengths())
    //         .map(move |(index, len)| A::sliced(array, *index as _, len));
    //
    //     let mut i = 0;
    //     izip!(self.indices(timeline).unwrap(), splits).filter(move |(index, batch)| {
    //         let is_valid = list_array.is_valid(i);
    //         i += 1;
    //         is_valid
    //     })
    // }
}

#[cfg(test)]
mod tests {
    use arrow2::datatypes::DataType as ArrowDatatype;
    use itertools::Itertools;
    use nohash_hasher::IntMap;

    use re_log_types::{
        build_frame_nr,
        example_components::{MyColor, MyLabel, MyPoint},
        EntityPath, ResolvedTimeRange, TimePoint,
    };
    use re_types_core::Loggable as _;

    use super::*;

    // TODO: we can def impl a range_indexed_raw that returns refs to the internal values() no?

    #[test]
    fn range_indexed() -> anyhow::Result<()> {
        let entity_path: EntityPath = "whatever".into();

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();

        let timepoint1 = [build_frame_nr(42)];
        let timepoint2 = [build_frame_nr(43)];
        let timepoint3 = [build_frame_nr(44)];

        let points1 = &[MyPoint::new(1.0, 1.0), MyPoint::new(2.0, 2.0)];
        let points3 = &[
            MyPoint::new(3.0, 3.0),
            MyPoint::new(4.0, 4.0),
            MyPoint::new(5.0, 5.0),
        ];

        let colors2 = &[MyColor::from_rgb(1, 1, 1)];

        let labels2 = &[
            MyLabel("a".into()),
            MyLabel("b".into()),
            MyLabel("c".into()),
        ];

        let chunk = Chunk::builder(entity_path)
            .with_component_batches(row_id3, timepoint1.clone(), [points3 as _])
            .with_component_batches(row_id1, timepoint2.clone(), [points1 as _])
            .with_component_batches(row_id2, timepoint3.clone(), [colors2 as _, labels2 as _])
            .build()?;
        eprintln!("{chunk}");

        dbg!(chunk
            .iter_indexed::<MyPoint>(&Timeline::new_sequence("frame_nr"))
            .collect_vec());
        dbg!(chunk
            .iter_indexed::<MyColor>(&Timeline::new_sequence("frame_nr"))
            .collect_vec());
        dbg!(chunk
            .iter_indexed::<MyLabel>(&Timeline::new_sequence("frame_nr"))
            .collect_vec());

        Ok(())
    }
}
