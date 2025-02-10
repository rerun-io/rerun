use arrow::{
    array::{Array as ArrowArray, ListArray as ArrowListArray, RecordBatch as ArrowRecordBatch},
    datatypes::Field as ArrowField,
};
use itertools::Itertools;
use nohash_hasher::IntMap;

use re_arrow_util::{arrow_util::into_arrow_ref, ArrowArrayDowncastRef as _};
use re_byte_size::SizeBytes as _;
use re_log_types::EntityPath;
use re_types_core::{arrow_helpers::as_array_ref, ComponentDescriptor, Loggable as _};

use crate::{chunk::ChunkComponents, Chunk, ChunkError, ChunkId, ChunkResult, RowId, TimeColumn};

pub type ArrowMetadata = std::collections::HashMap<String, String>;

// ---

/// A [`Chunk`] that is ready for transport.
///
/// It contains a schema with a matching number of columns, all of the same length.
///
/// This is just a wrapper around an [`ArrowRecordBatch`], with some helper functions on top.
/// It can be converted to and from [`ArrowRecordBatch`] without overhead.
///
/// Use the `Display` implementation to dump the chunk as a nicely formatted table.
///
/// This has a stable ABI! The entire point of this type is to allow users to send raw arrow data
/// into Rerun.
/// This means we have to be very careful when checking the validity of the data: slipping corrupt
/// data into the store could silently break all the index search logic (e.g. think of a chunk
/// claiming to be sorted while it is in fact not).
// TODO(emilk): remove this, and replace it with a trait extension type for `ArrowRecordBatch`.
#[derive(Debug, Clone)]
pub struct TransportChunk {}

// TODO(#6572): Relying on Arrow's native schema metadata feature is bound to fail, we need to
// switch to something more powerful asap.
impl TransportChunk {
    /// The key used to identify a Rerun [`ChunkId`] in chunk-level [`ArrowSchema`] metadata.
    pub const CHUNK_METADATA_KEY_ID: &'static str = "rerun.id";

    /// The key used to identify a Rerun [`EntityPath`] in chunk-level [`ArrowSchema`] metadata.
    pub const CHUNK_METADATA_KEY_ENTITY_PATH: &'static str = "rerun.entity_path";

    /// The key used to identify the size in bytes of the data, once loaded in memory, in chunk-level
    /// [`ArrowSchema`] metadata.
    pub const CHUNK_METADATA_KEY_HEAP_SIZE_BYTES: &'static str = "rerun.heap_size_bytes";

    /// The marker used to identify whether a chunk is sorted in chunk-level [`ArrowSchema`] metadata.
    ///
    /// The associated value is irrelevant -- if this marker is present, then it is true.
    ///
    /// Chunks are ascendingly sorted by their `RowId` column.
    pub const CHUNK_METADATA_MARKER_IS_SORTED_BY_ROW_ID: &'static str = "rerun.is_sorted";

    /// The key used to identify the kind of a Rerun column in field-level [`ArrowSchema`] metadata.
    ///
    /// That is: control columns (e.g. `row_id`), time columns or component columns.
    pub const FIELD_METADATA_KEY_KIND: &'static str = "rerun.kind";

    /// The value used to identify a Rerun time column in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_VALUE_KIND_TIME: &'static str = "time";

    /// The value used to identify a Rerun control column in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_VALUE_KIND_CONTROL: &'static str = "control";

    /// The value used to identify a Rerun data column in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_VALUE_KIND_DATA: &'static str = "data";

    /// The key used to identify the [`crate::ArchetypeName`] in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_KEY_ARCHETYPE_NAME: &'static str = "rerun.archetype_name";

    /// The key used to identify the [`crate::ArchetypeFieldName`] in field-level [`ArrowSchema`] metadata.
    pub const FIELD_METADATA_KEY_ARCHETYPE_FIELD_NAME: &'static str = "rerun.archetype_field_name";

    /// The marker used to identify whether a column is sorted in field-level [`ArrowSchema`] metadata.
    ///
    /// The associated value is irrelevant -- if this marker is present, then it is true.
    ///
    /// Chunks are ascendingly sorted by their `RowId` column but, depending on whether the data
    /// was logged out of order or not for a given time column, that column might follow the global
    /// `RowId` while still being unsorted relative to its own time order.
    pub const FIELD_METADATA_MARKER_IS_SORTED_BY_TIME: &'static str =
        Self::CHUNK_METADATA_MARKER_IS_SORTED_BY_ROW_ID;

    /// Returns the appropriate chunk-level [`ArrowSchema`] metadata for a Rerun [`ChunkId`].
    #[inline]
    pub fn chunk_metadata_id(id: ChunkId) -> ArrowMetadata {
        [
            (
                Self::CHUNK_METADATA_KEY_ID.to_owned(),
                format!("{:X}", id.as_u128()),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate chunk-level [`ArrowSchema`] metadata for the in-memory size in bytes.
    #[inline]
    pub fn chunk_metadata_heap_size_bytes(heap_size_bytes: u64) -> ArrowMetadata {
        [
            (
                Self::CHUNK_METADATA_KEY_HEAP_SIZE_BYTES.to_owned(),
                heap_size_bytes.to_string(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate chunk-level [`ArrowSchema`] metadata for a Rerun [`EntityPath`].
    #[inline]
    pub fn chunk_metadata_entity_path(entity_path: &EntityPath) -> ArrowMetadata {
        [
            (
                Self::CHUNK_METADATA_KEY_ENTITY_PATH.to_owned(),
                entity_path.to_string(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate chunk-level [`ArrowSchema`] metadata for an `IS_SORTED` marker.
    #[inline]
    pub fn chunk_metadata_is_sorted() -> ArrowMetadata {
        [
            (
                Self::CHUNK_METADATA_MARKER_IS_SORTED_BY_ROW_ID.to_owned(),
                String::new(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate field-level [`ArrowSchema`] metadata for a Rerun time column.
    #[inline]
    pub fn field_metadata_time_column() -> ArrowMetadata {
        [
            (
                Self::FIELD_METADATA_KEY_KIND.to_owned(),
                Self::FIELD_METADATA_VALUE_KIND_TIME.to_owned(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate field-level [`ArrowSchema`] metadata for a Rerun control column.
    #[inline]
    pub fn field_metadata_control_column() -> ArrowMetadata {
        [
            (
                Self::FIELD_METADATA_KEY_KIND.to_owned(),
                Self::FIELD_METADATA_VALUE_KIND_CONTROL.to_owned(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate field-level [`ArrowSchema`] metadata for a Rerun data column.
    #[inline]
    pub fn field_metadata_data_column() -> ArrowMetadata {
        [
            (
                Self::FIELD_METADATA_KEY_KIND.to_owned(),
                Self::FIELD_METADATA_VALUE_KIND_DATA.to_owned(),
            ), //
        ]
        .into()
    }

    /// Returns the appropriate field-level [`ArrowSchema`] metadata for an `IS_SORTED` marker.
    #[inline]
    pub fn field_metadata_is_sorted() -> ArrowMetadata {
        [
            (
                Self::FIELD_METADATA_MARKER_IS_SORTED_BY_TIME.to_owned(),
                String::new(),
            ), //
        ]
        .into()
    }

    #[inline]
    pub fn field_metadata_component_descriptor(
        component_desc: &ComponentDescriptor,
    ) -> ArrowMetadata {
        component_desc
            .archetype_name
            .iter()
            .copied()
            .map(|archetype_name| {
                (
                    Self::FIELD_METADATA_KEY_ARCHETYPE_NAME.to_owned(),
                    archetype_name.to_string(),
                )
            })
            .chain(component_desc.archetype_field_name.iter().copied().map(
                |archetype_field_name| {
                    (
                        Self::FIELD_METADATA_KEY_ARCHETYPE_FIELD_NAME.to_owned(),
                        archetype_field_name.to_string(),
                    )
                },
            ))
            .collect()
    }

    #[inline]
    pub fn component_descriptor_from_field(field: &ArrowField) -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: field
                .metadata()
                .get(Self::FIELD_METADATA_KEY_ARCHETYPE_NAME)
                .cloned()
                .map(Into::into),
            component_name: field.name().clone().into(),
            archetype_field_name: field
                .metadata()
                .get(Self::FIELD_METADATA_KEY_ARCHETYPE_FIELD_NAME)
                .cloned()
                .map(Into::into),
        }
    }
}

impl Chunk {
    /// Prepare the [`Chunk`] for transport.
    ///
    /// It is probably a good idea to sort the chunk first.
    pub fn to_chunk_batch(&self) -> ChunkResult<re_sorbet::ChunkBatch> {
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

        let row_id_schema = re_sorbet::RowIdColumnDescriptor::try_from(RowId::arrow_datatype())?;

        let (index_schemas, index_arrays): (Vec<_>, Vec<_>) = {
            re_tracing::profile_scope!("timelines");

            let mut timelines = timelines
                .iter()
                .map(|(timeline, info)| {
                    let TimeColumn {
                        timeline: _,
                        times: _,
                        is_sorted,
                        time_range: _,
                    } = info;

                    let array = info.times_array();
                    let schema = re_sorbet::TimeColumnDescriptor {
                        timeline: *timeline,
                        datatype: array.data_type().clone(),
                        is_sorted: *is_sorted,
                    };

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
                .flat_map(|per_desc| per_desc.iter())
                .map(|(component_desc, list_array)| {
                    let list_array = ArrowListArray::from(list_array.clone());
                    let ComponentDescriptor {
                        archetype_name,
                        archetype_field_name,
                        component_name,
                    } = *component_desc;

                    let schema = re_sorbet::ComponentColumnDescriptor {
                        store_datatype: list_array.data_type().clone(),
                        entity_path: entity_path.clone(),

                        archetype_name,
                        archetype_field_name,
                        component_name,

                        is_static: false,             // TODO
                        is_indicator: false,          // TODO
                        is_tombstone: false,          // TODO
                        is_semantically_empty: false, // TODO
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
        )
        .with_heap_size_bytes(heap_size_bytes)
        .with_sorted(*is_sorted);

        Ok(re_sorbet::ChunkBatch::try_new(
            schema,
            into_arrow_ref(row_ids.clone()),
            index_arrays,
            data_arrays,
        )?)
    }
}

impl Chunk {
    /// Prepare the [`Chunk`] for transport.
    ///
    /// It is probably a good idea to sort the chunk first.
    pub fn to_record_batch(&self) -> ChunkResult<ArrowRecordBatch> {
        Ok(self.to_chunk_batch()?.into())
    }

    pub fn from_chunk_batch(batch: &re_sorbet::ChunkBatch) -> ChunkResult<Self> {
        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            batch.num_columns(),
            batch.num_rows()
        ));

        // Metadata
        let (id, entity_path, is_sorted) = (
            batch.chunk_id(),
            batch.entity_path().clone(),
            batch.is_sorted(),
        );

        let row_ids = batch.row_id_column().1.clone();

        let timelines = {
            re_tracing::profile_scope!("timelines");

            let mut timelines = IntMap::default();

            for (schema, column) in batch.index_columns() {
                let timeline = schema.timeline();

                let times =
                    TimeColumn::read_array(&as_array_ref(column.clone())).map_err(|err| {
                        ChunkError::Malformed {
                            reason: format!("Bad time column '{}': {err}", schema.name()),
                        }
                    })?;

                let time_column =
                    TimeColumn::new(schema.is_sorted.then_some(true), timeline, times);
                if timelines.insert(timeline, time_column).is_some() {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "time column '{}' was specified more than once",
                            schema.name(),
                        ),
                    });
                }
            }

            timelines
        };

        let components = {
            let mut components = ChunkComponents::default();

            for (schema, column) in batch.data_columns() {
                let column = column
                    .downcast_array_ref::<ArrowListArray>()
                    .ok_or_else(|| ChunkError::Malformed {
                        reason: format!(
                            "The outer array in a chunked component batch must be a sparse list, got {:?}",
                            column.data_type(),
                        ),
                    })?;

                let component_desc = ComponentDescriptor {
                    archetype_name: schema.archetype_name,
                    archetype_field_name: schema.archetype_field_name,
                    component_name: schema.component_name,
                };

                if components
                    .insert_descriptor(component_desc, column.clone())
                    .is_some()
                {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "component column '{schema:?}' was specified more than once"
                        ),
                    });
                }
            }

            components
        };

        let mut res = Self::new(
            id,
            entity_path,
            is_sorted.then_some(true),
            row_ids,
            timelines,
            components,
        )?;

        if let Some(heap_size_bytes) = batch.heap_size_bytes() {
            res.heap_size_bytes = heap_size_bytes.into();
        }

        Ok(res)
    }

    pub fn from_record_batch(batch: ArrowRecordBatch) -> ChunkResult<Self> {
        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            batch.num_columns(),
            batch.num_rows()
        ));
        Self::from_chunk_batch(&re_sorbet::ChunkBatch::try_from(batch)?)
    }
}

impl Chunk {
    #[inline]
    pub fn from_arrow_msg(msg: &re_log_types::ArrowMsg) -> ChunkResult<Self> {
        let re_log_types::ArrowMsg {
            chunk_id: _,
            timepoint_max: _,
            batch,
            on_release: _,
        } = msg;

        Self::from_record_batch(batch.clone())
    }

    #[inline]
    pub fn to_arrow_msg(&self) -> ChunkResult<re_log_types::ArrowMsg> {
        re_tracing::profile_function!();
        self.sanity_check()?;

        Ok(re_log_types::ArrowMsg {
            chunk_id: re_tuid::Tuid::from_u128(self.id().as_u128()),
            timepoint_max: self.timepoint_max(),
            batch: self.to_record_batch()?,
            on_release: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use nohash_hasher::IntMap;
    use similar_asserts::assert_eq;

    use re_arrow_util::arrow_util;
    use re_log_types::{
        example_components::{MyColor, MyPoint},
        Timeline,
    };
    use re_types_core::Component as _;

    use super::*;

    #[test]
    fn roundtrip() -> anyhow::Result<()> {
        let entity_path = EntityPath::parse_forgiving("a/b/c");

        let timeline1 = Timeline::new_temporal("log_time");
        let timelines1: IntMap<_, _> = std::iter::once((
            timeline1,
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
            (MyPoint::descriptor(), {
                let list_array = arrow_util::arrays_to_list_array_opt(&[
                    Some(&*points1),
                    points2,
                    Some(&*points3),
                    Some(&*points4),
                ])
                .unwrap();
                assert_eq!(4, list_array.len());
                list_array
            }),
            (MyPoint::descriptor(), {
                let list_array = arrow_util::arrays_to_list_array_opt(&[
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

            let chunk_batch_after = re_sorbet::ChunkBatch::try_from(arrow_record_batch).unwrap();

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
