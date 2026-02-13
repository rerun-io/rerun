use arrow::array::{Array as _, ListArray as ArrowListArray, RecordBatch as ArrowRecordBatch};
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_arrow_util::{ArrowArrayDowncastRef as _, into_arrow_ref};
use re_byte_size::SizeBytes as _;
use re_types_core::arrow_helpers::as_array_ref;
use re_types_core::{ComponentDescriptor, SerializedComponentColumn};

use crate::chunk::ChunkComponents;
use crate::{Chunk, ChunkError, ChunkResult, TimeColumn};

// ---

impl Chunk {
    /// Prepare the [`Chunk`] for transport.
    ///
    /// It is probably a good idea to sort the chunk first.
    // TODO(#8744): this is infallible, so we should not return a `Result` here.
    pub fn to_record_batch(&self) -> ChunkResult<ArrowRecordBatch> {
        re_tracing::profile_function!();
        Ok(self.to_chunk_batch()?.into())
    }

    /// Prepare the [`Chunk`] for transport.
    ///
    /// It is probably a good idea to sort the chunk first.
    // TODO(#8744): this is infallible, so we should not return a `Result` here.
    pub fn to_chunk_batch(&self) -> ChunkResult<re_sorbet::ChunkBatch> {
        re_tracing::profile_function!();
        self.sanity_check()?;

        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            self.num_columns(),
            self.num_rows()
        ));

        let heap_size_bytes = self.heap_size_bytes();
        let Self {
            id,
            entity_path,
            heap_size_bytes: _, // use the method instead because of lazy initialization
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let row_id_schema = re_sorbet::RowIdColumnDescriptor {
            is_sorted: *is_sorted,
        };

        let (index_schemas, index_arrays): (Vec<_>, Vec<_>) = {
            re_tracing::profile_scope!("timelines");

            let mut timelines = timelines
                .values()
                .map(|info| {
                    let TimeColumn {
                        timeline,
                        times: _,
                        is_sorted,
                        time_range: _,
                    } = info;

                    let array = info.times_array();

                    re_log::debug_assert_eq!(&timeline.datatype(), array.data_type());

                    let schema =
                        re_sorbet::IndexColumnDescriptor::from_timeline(*timeline, *is_sorted);

                    (schema, into_arrow_ref(array))
                })
                .collect_vec();

            timelines.sort_by(|(schema_a, _), (schema_b, _)| schema_a.cmp(schema_b));

            timelines.into_iter().unzip()
        };

        let (data_schemas, data_arrays): (Vec<_>, Vec<_>) = {
            re_tracing::profile_scope!("components");

            let mut components = components
                .values()
                .map(|column| {
                    let SerializedComponentColumn {
                        list_array,
                        descriptor:
                            ComponentDescriptor {
                                archetype,
                                component,
                                component_type,
                            },
                    } = column.clone();

                    if let Some(c) = component_type {
                        c.sanity_check();
                    }

                    let schema = re_sorbet::ComponentColumnDescriptor {
                        store_datatype: list_array.data_type().clone(),
                        entity_path: entity_path.clone(),

                        archetype,
                        component,
                        component_type,

                        // These are a consequence of using `ComponentColumnDescriptor` both for chunk batches and dataframe batches.
                        // Setting them all to `false` at least ensures they aren't written to the arrow metadata:
                        // TODO(#8744): figure out what to do here
                        is_static: false,
                        is_tombstone: false,
                        is_semantically_empty: false,
                    };
                    (schema, into_arrow_ref(list_array))
                })
                .collect_vec();

            components.sort_by(|(schema_a, _), (schema_b, _)| schema_a.cmp(schema_b));

            components.into_iter().unzip()
        };

        let schema = re_sorbet::ChunkSchema::new(
            *id,
            entity_path.clone(),
            row_id_schema,
            index_schemas,
            data_schemas,
            Default::default(),
        )
        .with_heap_size_bytes(heap_size_bytes);

        Ok(re_sorbet::ChunkBatch::try_new(
            schema,
            into_arrow_ref(row_ids.clone()),
            index_arrays,
            data_arrays,
        )?)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub fn from_record_batch(batch: &ArrowRecordBatch) -> ChunkResult<Self> {
        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            batch.num_columns(),
            batch.num_rows()
        ));
        Self::from_chunk_batch(&re_sorbet::ChunkBatch::try_from(batch)?)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub fn from_chunk_batch(batch: &re_sorbet::ChunkBatch) -> ChunkResult<Self> {
        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            batch.num_columns(),
            batch.num_rows()
        ));

        let row_ids = batch.row_id_column().1.clone();

        let timelines = {
            re_tracing::profile_scope!("timelines");

            let mut timelines = IntMap::default();

            for (schema, column) in batch.index_columns() {
                let timeline = schema.timeline();

                let times =
                    TimeColumn::read_array(&as_array_ref(column.clone())).map_err(|err| {
                        ChunkError::Malformed {
                            reason: format!("Bad time column '{}': {err}", schema.column_name()),
                        }
                    })?;

                let time_column =
                    TimeColumn::new(schema.is_sorted().then_some(true), timeline, times);
                if timelines.insert(*timeline.name(), time_column).is_some() {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "time column '{}' was specified more than once",
                            timeline.name()
                        ),
                    });
                }
            }

            timelines
        };

        let components = {
            let mut components = ChunkComponents::default();

            for (schema, column) in batch.component_columns() {
                let column = column
                    .downcast_array_ref::<ArrowListArray>()
                    .ok_or_else(|| ChunkError::Malformed {
                        reason: format!(
                            "The outer array in a chunked component batch must be a sparse list, got {:?}",
                            column.data_type(),
                        ),
                    })?;

                let component_desc = ComponentDescriptor {
                    archetype: schema.archetype,
                    component: schema.component,
                    component_type: schema.component_type,
                };

                if components
                    .insert(SerializedComponentColumn::new(
                        column.clone(),
                        component_desc,
                    ))
                    .is_some()
                {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "component column '{:?}' was specified more than once",
                            schema.component,
                        ),
                    });
                }
            }

            components
        };

        let is_sorted_by_row_id = if batch.chunk_schema().row_id_column().is_sorted {
            Some(true) // trust the chunk schema
        } else {
            None // Check whether or not it is sorted
        };

        let mut res = Self::new(
            batch.chunk_id(),
            batch.entity_path().clone(),
            is_sorted_by_row_id,
            row_ids,
            timelines,
            components,
        )?;

        if let Some(heap_size_bytes) = batch.heap_size_bytes() {
            res.heap_size_bytes = heap_size_bytes.into();
        }

        Ok(res)
    }
}

impl Chunk {
    #[inline]
    pub fn from_arrow_msg(msg: &re_log_types::ArrowMsg) -> ChunkResult<Self> {
        let re_log_types::ArrowMsg {
            chunk_id: _,
            batch,
            on_release: _,
        } = msg;

        Self::from_record_batch(batch)
    }

    #[inline]
    pub fn to_arrow_msg(&self) -> ChunkResult<re_log_types::ArrowMsg> {
        re_tracing::profile_function!();
        self.sanity_check()?;

        Ok(re_log_types::ArrowMsg {
            chunk_id: self.id().as_tuid(),
            batch: self.to_record_batch()?,
            on_release: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use nohash_hasher::IntMap;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{EntityPath, Timeline};
    use re_types_core::{ChunkId, Loggable as _, RowId};
    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn roundtrip() -> anyhow::Result<()> {
        let entity_path = EntityPath::parse_forgiving("a/b/c");

        let timeline1 = Timeline::new_duration("log_time");
        let timelines1: IntMap<_, _> = std::iter::once((
            *timeline1.name(),
            TimeColumn::new(Some(true), timeline1, vec![42, 43, 44, 45].into()),
        ))
        .collect();

        let timelines2 = IntMap::default(); // static

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
        let colors4 = None;

        let components = [
            (MyPoints::descriptor_points(), {
                let list_array = re_arrow_util::arrays_to_list_array_opt(&[
                    Some(&*points1),
                    points2,
                    Some(&*points3),
                    Some(&*points4),
                ])
                .unwrap();
                assert_eq!(4, list_array.len());
                list_array
            }),
            (MyPoints::descriptor_points(), {
                let list_array = re_arrow_util::arrays_to_list_array_opt(&[
                    Some(&*colors1),
                    Some(&*colors2),
                    colors3,
                    colors4,
                ])
                .unwrap();
                assert_eq!(4, list_array.len());
                list_array
            }),
        ];

        let row_ids = vec![RowId::new(), RowId::new(), RowId::new(), RowId::new()];

        for timelines in [timelines1, timelines2] {
            let chunk_before = Chunk::from_native_row_ids(
                ChunkId::new(),
                entity_path.clone(),
                None,
                &row_ids,
                timelines.clone(),
                components.clone().into_iter().collect(),
            )
            .unwrap();

            let chunk_batch_before = chunk_before.to_chunk_batch().unwrap();

            assert_eq!(chunk_before.num_columns(), chunk_batch_before.num_columns());
            assert_eq!(chunk_before.num_rows(), chunk_batch_before.num_rows());

            let arrow_record_batch = ArrowRecordBatch::from(&chunk_batch_before);

            let chunk_batch_after = re_sorbet::ChunkBatch::try_from(&arrow_record_batch).unwrap();

            assert_eq!(
                chunk_batch_before.chunk_schema(),
                chunk_batch_after.chunk_schema()
            );
            assert_eq!(chunk_batch_before.num_rows(), chunk_batch_after.num_rows());

            let chunk_after = Chunk::from_chunk_batch(&chunk_batch_after).unwrap();

            assert_eq!(chunk_before.entity_path(), chunk_after.entity_path());
            assert_eq!(
                chunk_before.heap_size_bytes(),
                chunk_after.heap_size_bytes(),
            );
            assert_eq!(chunk_before.num_columns(), chunk_after.num_columns());
            assert_eq!(chunk_before.num_rows(), chunk_after.num_rows());
            assert!(chunk_before.are_equal(&chunk_after));
            assert_eq!(chunk_before, chunk_after);
        }

        Ok(())
    }
}
