use std::sync::Arc;

use arrow2::{
    array::{Array as ArrowArray, PrimitiveArray},
    Either,
};
use itertools::Itertools as _;

use re_log_types::{TimeInt, Timeline};
use re_types_core::{Component, ComponentName};

use crate::{Chunk, ChunkTimeline, RowId};

// TODO: these really need tests

// ---

impl Chunk {
    /// Returns an iterator over the rows of the [`Chunk`].
    ///
    /// Each yielded item is a component batch with its associated index ([`RowId`] + data time).
    ///
    /// Iterating a [`Chunk`] on a row basis is very wasteful, performance-wise.
    /// Prefer columnar access when possible.
    //
    // TODO: a row-based iterator is obviously not what we want -- one of the benefits of
    // chunks is to amortize the cost of downcasting & "deserialization".
    // But at the moment we still need to run with the native deserialization cache, which expects
    // row-based data.
    // As soon as we remove the native cache and start exposing `Chunk`s directly to downstream
    // systems, we will look into ergonomic ways to do columnar access.
    //
    // TODO: this is probably wrong too
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
}

pub struct ChunkIndicesIter {
    chunk: Arc<Chunk>,

    time_chunk: Option<ChunkTimeline>,
    index: usize,
}

impl Iterator for ChunkIndicesIter {
    type Item = (TimeInt, RowId);

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.index;
        self.index += 1;

        let row_id = {
            let (times, incs) = self.chunk.row_ids_raw();
            let times = times.values().as_slice();
            let incs = incs.values().as_slice();

            let time = *times.get(i)?;
            let inc = *incs.get(i)?;

            RowId::from_u128(((time as u128) << 64) | (inc as u128))
        };

        if let Some(time_chunk) = &self.time_chunk {
            let time = *time_chunk.times_raw().get(i)?;
            let time = TimeInt::new_temporal(time);
            Some((time, row_id))
        } else {
            Some((TimeInt::STATIC, row_id))
        }
    }
}

impl Chunk {
    /// Returns an iterator over the indices (`(TimeInt, RowId)`) of a [`Chunk`], for a given timeline.
    ///
    /// If the chunk is static, `timeline` will be ignored.
    ///
    /// The returned iterator outlives `self`, thus it can be passed around freely.
    #[inline]
    pub fn iter_indices(
        self: Arc<Self>,
        timeline: &Timeline,
    ) -> impl Iterator<Item = (TimeInt, RowId)> {
        if self.is_static() {
            Either::Left(ChunkIndicesIter {
                chunk: self,
                time_chunk: None,
                index: 0,
            })
        } else {
            self.timelines.get(timeline).cloned().map_or_else(
                || Either::Right(Either::Left(std::iter::empty())),
                |time_chunk| {
                    Either::Right(Either::Right(ChunkIndicesIter {
                        chunk: self,
                        time_chunk: Some(time_chunk),
                        index: 0,
                    }))
                },
            )
        }
    }

    /// Returns an iterator over the indices (`(TimeInt, RowId)`) of a [`Chunk`], for a given timeline.
    ///
    /// If the chunk is static, `timeline` will be ignored.
    ///
    /// The returned iterator outlives `self`, thus it can be passed around freely.
    //
    // TODO: explain why we have that one too
    #[inline]
    pub fn iter_component_indices(
        &self,
        timeline: &Timeline,
        component_name: &ComponentName,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + '_ {
        let Some(list_array) = self.components.get(component_name) else {
            return Either::Left(std::iter::empty());
        };

        if self.is_static() {
            let indices = itertools::izip!(std::iter::repeat(TimeInt::STATIC), self.row_ids());

            if let Some(validity) = list_array.validity() {
                Either::Right(Either::Left(Either::Left(
                    indices
                        .enumerate()
                        .filter_map(|(i, o)| validity.get_bit(i).then_some(o)),
                )))
            } else {
                Either::Right(Either::Left(Either::Right(indices)))
            }
        } else {
            let Some(time_chunk) = self.timelines.get(timeline) else {
                return Either::Left(std::iter::empty());
            };

            let indices = itertools::izip!(time_chunk.times(), self.row_ids());

            if let Some(validity) = list_array.validity() {
                Either::Right(Either::Right(Either::Left(
                    indices
                        .enumerate()
                        .filter_map(|(i, o)| validity.get_bit(i).then_some(o)),
                )))
            } else {
                Either::Right(Either::Right(Either::Right(indices)))
            }
        }
    }

    #[inline]
    pub fn iter_primitive<T: arrow2::types::NativeType>(
        &self,
        component_name: &ComponentName,
    ) -> impl Iterator<Item = ((usize, usize), &[T])> + '_ {
        let Some(list_array) = self.components.get(component_name) else {
            return Either::Left(std::iter::empty());
        };

        let Some(values) = list_array
            .values()
            .as_any()
            .downcast_ref::<PrimitiveArray<T>>()
        else {
            // TODO: warn
            todo!();
            return Either::Left(std::iter::empty());
        };
        let values = values.values().as_slice();

        // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
        Either::Right(
            self.iter_offsets(component_name)
                .map(move |(idx, len)| ((idx, len), &values[idx..idx + len])),
        )
    }

    // TODO
    pub fn iter_offsets(
        &self,
        component_name: &ComponentName,
    ) -> impl Iterator<Item = (usize, usize)> + '_ {
        // TODO: let's imagine we're densified and sorted and everything -- what now?
        // TODO: what if we're static though
        // TODO: unless maybe we want both?

        let Some(list_array) = self.components.get(component_name) else {
            return Either::Left(std::iter::empty());
        };

        let offsets = list_array.offsets().iter().map(|idx| *idx as usize);
        let lengths = list_array.offsets().lengths();

        if let Some(validity) = list_array.validity() {
            Either::Right(Either::Left(
                itertools::izip!(offsets, lengths)
                    .enumerate()
                    .filter_map(|(i, o)| validity.get_bit(i).then_some(o)),
            ))
        } else {
            Either::Right(Either::Right(itertools::izip!(offsets, lengths)))
        }
    }

    // TODO
    #[cfg(TODO)]
    pub fn for_each<F>(&self, timeline: &Timeline, component_name: &ComponentName, f: F)
    where
        F: FnMut((TimeInt, RowId), (usize, usize)),
    {
        // TODO: let's imagine we're densified and sorted and everything -- what now?
        // TODO: what if we're static though
        // TODO: unless maybe we want both?

        let Some(list_array) = self.components.get(component_name) else {
            return;
        };

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
}

pub struct ChunkComponentIter<C, IO> {
    values: Vec<C>,
    offsets: IO,
}

pub struct ChunkComponentIterRef<'a, C, IO> {
    values: &'a [C],
    offsets: &'a mut IO,
}

impl<'a, C: Component, IO: Iterator<Item = (usize, usize)>> IntoIterator
    for &'a mut ChunkComponentIter<C, IO>
{
    type Item = ((usize, usize), &'a [C]);

    type IntoIter = ChunkComponentIterRef<'a, C, IO>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ChunkComponentIterRef {
            values: &self.values,
            offsets: &mut self.offsets,
        }
    }
}

impl<'a, C: Component, IO: Iterator<Item = (usize, usize)>> Iterator
    for ChunkComponentIterRef<'a, C, IO>
{
    type Item = ((usize, usize), &'a [C]);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.offsets
            .next()
            .map(move |(idx, len)| ((idx, len), &self.values[idx..idx + len]))
    }
}

impl Chunk {
    #[inline]
    pub fn iter_component<C: Component>(
        &self,
    ) -> ChunkComponentIter<C, impl Iterator<Item = (usize, usize)> + '_> {
        let Some(list_array) = self.components.get(&C::name()) else {
            return ChunkComponentIter {
                values: vec![],
                offsets: Either::Left(std::iter::empty()),
            };
        };

        // TODO: what do with error? just log?
        let values = list_array.values();
        let Ok(values) = C::from_arrow(&**values) else {
            // TODO: warn
            todo!();
            return ChunkComponentIter {
                values: vec![],
                offsets: Either::Left(std::iter::empty()),
            };
        };

        // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
        ChunkComponentIter {
            values,
            offsets: Either::Right(self.iter_offsets(&C::name())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use itertools::{izip, Itertools};
    use re_log_types::{
        example_components::{MyColor, MyLabel, MyPoint},
        EntityPath, TimeInt, TimePoint,
    };
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

    #[test]
    fn iter_indices_temporal() -> anyhow::Result<()> {
        let entity_path = EntityPath::from("this/that");

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timeline_frame = Timeline::new_sequence("frame");

        let timepoint1 = [(timeline_frame, 1)];
        let timepoint2 = [(timeline_frame, 3)];
        let timepoint3 = [(timeline_frame, 5)];
        let timepoint4 = [(timeline_frame, 7)];
        let timepoint5 = [(timeline_frame, 9)];

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(3.0, 3.0)];
        let points4 = &[MyPoint::new(4.0, 4.0)];
        let points5 = &[MyPoint::new(5.0, 5.0)];

        let chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batches(row_id1, timepoint1, [points1 as _])
                .with_component_batches(row_id2, timepoint2, [points2 as _])
                .with_component_batches(row_id3, timepoint3, [points3 as _])
                .with_component_batches(row_id4, timepoint4, [points4 as _])
                .with_component_batches(row_id5, timepoint5, [points5 as _])
                .build()?,
        );

        {
            let got = Arc::clone(&chunk)
                .iter_indices(&timeline_frame)
                .map(|it| it.collect_vec())
                .unwrap_or_default();
            let expected = izip!(
                chunk
                    .timelines
                    .get(&timeline_frame)
                    .map(|time_chunk| time_chunk.times().collect_vec())
                    .unwrap_or_default(),
                chunk.row_ids()
            )
            .collect_vec();

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    #[test]
    fn iter_indices_static() -> anyhow::Result<()> {
        let entity_path = EntityPath::from("this/that");

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timeline_frame = Timeline::new_sequence("frame");

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(3.0, 3.0)];
        let points4 = &[MyPoint::new(4.0, 4.0)];
        let points5 = &[MyPoint::new(5.0, 5.0)];

        let chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batches(row_id1, TimePoint::default(), [points1 as _])
                .with_component_batches(row_id2, TimePoint::default(), [points2 as _])
                .with_component_batches(row_id3, TimePoint::default(), [points3 as _])
                .with_component_batches(row_id4, TimePoint::default(), [points4 as _])
                .with_component_batches(row_id5, TimePoint::default(), [points5 as _])
                .build()?,
        );

        {
            let got = Arc::clone(&chunk)
                .iter_indices(&timeline_frame)
                .map(|it| it.collect_vec())
                .unwrap_or_default();
            let expected = izip!(std::iter::repeat(TimeInt::STATIC), chunk.row_ids()).collect_vec();

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }
}
