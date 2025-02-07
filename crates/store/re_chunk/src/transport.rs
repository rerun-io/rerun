use arrow::{
    array::{
        Array as ArrowArray, ArrayRef as ArrowArrayRef, ListArray as ArrowListArray,
        RecordBatch as ArrowRecordBatch, StructArray as ArrowStructArray,
    },
    datatypes::{
        DataType as ArrowDatatype, Field as ArrowField, Fields as ArrowFields,
        Schema as ArrowSchema, TimeUnit as ArrowTimeUnit,
    },
};
use itertools::Itertools;
use nohash_hasher::IntMap;
use tap::Tap as _;

use re_arrow_util::{arrow_util::into_arrow_ref, ArrowArrayDowncastRef as _};
use re_byte_size::SizeBytes as _;
use re_log_types::{EntityPath, Timeline};
use re_types_core::{
    arrow_helpers::as_array_ref, Component as _, ComponentDescriptor, Loggable as _,
};

use crate::{chunk::ChunkComponents, Chunk, ChunkError, ChunkId, ChunkResult, RowId, TimeColumn};

pub type ArrowMetadata = std::collections::HashMap<String, String>;

// ---

/// A [`Chunk`] that is ready for transport. Obtained by calling [`Chunk::to_transport`].
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
pub struct TransportChunk {
    batch: ArrowRecordBatch,
}

impl std::fmt::Display for TransportChunk {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        re_format_arrow::format_record_batch_with_width(self, f.width()).fmt(f)
    }
}

impl AsRef<ArrowRecordBatch> for TransportChunk {
    #[inline]
    fn as_ref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl std::ops::Deref for TransportChunk {
    type Target = ArrowRecordBatch;

    #[inline]
    fn deref(&self) -> &ArrowRecordBatch {
        &self.batch
    }
}

impl From<ArrowRecordBatch> for TransportChunk {
    #[inline]
    fn from(batch: ArrowRecordBatch) -> Self {
        Self { batch }
    }
}

impl From<TransportChunk> for ArrowRecordBatch {
    #[inline]
    fn from(chunk: TransportChunk) -> Self {
        chunk.batch
    }
}

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

impl TransportChunk {
    #[inline]
    pub fn id(&self) -> ChunkResult<ChunkId> {
        if let Some(id) = self.metadata().get(Self::CHUNK_METADATA_KEY_ID) {
            let id = u128::from_str_radix(id, 16).map_err(|err| ChunkError::Malformed {
                reason: format!("cannot deserialize chunk id: {err}"),
            })?;
            Ok(ChunkId::from_u128(id))
        } else {
            Err(crate::ChunkError::Malformed {
                reason: format!("chunk id missing from metadata ({:?})", self.metadata()),
            })
        }
    }

    #[inline]
    pub fn entity_path(&self) -> ChunkResult<EntityPath> {
        match self
            .schema_ref()
            .metadata
            .get(Self::CHUNK_METADATA_KEY_ENTITY_PATH)
        {
            Some(entity_path) => Ok(EntityPath::parse_forgiving(entity_path)),
            None => Err(crate::ChunkError::Malformed {
                reason: format!(
                    "entity path missing from metadata ({:?})",
                    self.schema_ref().metadata
                ),
            }),
        }
    }

    /// The size in bytes of the data, once loaded in memory, in chunk-level.
    ///
    /// This is stored in the metadata. Returns `None` if that key is not set.
    #[inline]
    pub fn heap_size_bytes(&self) -> Option<u64> {
        self.metadata()
            .get(Self::CHUNK_METADATA_KEY_HEAP_SIZE_BYTES)
            .and_then(|s| s.parse::<u64>().ok())
    }

    #[inline]
    pub fn fields(&self) -> &ArrowFields {
        &self.schema_ref().fields
    }

    pub fn metadata(&self) -> &std::collections::HashMap<String, String> {
        &self.batch.schema_ref().metadata
    }

    /// Looks in the chunk metadata for the `IS_SORTED` marker.
    ///
    /// It is possible that a chunk is sorted but didn't set that marker.
    /// This is fine, although wasteful.
    #[inline]
    pub fn is_sorted(&self) -> bool {
        self.metadata()
            .contains_key(Self::CHUNK_METADATA_MARKER_IS_SORTED_BY_ROW_ID)
    }

    /// Iterates all columns of the specified `kind`.
    ///
    /// See:
    /// * [`Self::FIELD_METADATA_VALUE_KIND_TIME`]
    /// * [`Self::FIELD_METADATA_VALUE_KIND_CONTROL`]
    /// * [`Self::FIELD_METADATA_VALUE_KIND_DATA`]
    #[inline]
    fn columns_of_kind<'a>(
        &'a self,
        kind: &'a str,
    ) -> impl Iterator<Item = (&'a ArrowField, &'a ArrowArrayRef)> + 'a {
        self.fields().iter().enumerate().filter_map(|(i, field)| {
            let actual_kind = field.metadata().get(Self::FIELD_METADATA_KEY_KIND);
            (actual_kind.map(|s| s.as_str()) == Some(kind))
                .then(|| {
                    self.batch
                        .columns()
                        .get(i)
                        .map(|column| (field.as_ref(), column))
                })
                .flatten()
        })
    }

    /// Iterates all control columns present in this chunk.
    #[inline]
    pub fn controls(&self) -> impl Iterator<Item = (&ArrowField, &ArrowArrayRef)> {
        self.columns_of_kind(Self::FIELD_METADATA_VALUE_KIND_CONTROL)
    }

    /// Iterates all data columns present in this chunk.
    #[inline]
    pub fn components(&self) -> impl Iterator<Item = (&ArrowField, &ArrowArrayRef)> {
        self.columns_of_kind(Self::FIELD_METADATA_VALUE_KIND_DATA)
    }

    /// Iterates all timeline columns present in this chunk.
    #[inline]
    pub fn timelines(&self) -> impl Iterator<Item = (&ArrowField, &ArrowArrayRef)> {
        self.columns_of_kind(Self::FIELD_METADATA_VALUE_KIND_TIME)
    }

    #[inline]
    pub fn num_controls(&self) -> usize {
        self.controls().count()
    }

    #[inline]
    pub fn num_timelines(&self) -> usize {
        self.timelines().count()
    }

    #[inline]
    pub fn num_components(&self) -> usize {
        self.components().count()
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
        self.sanity_check()?;

        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            self.num_columns(),
            self.num_rows()
        ));

        let Self {
            id,
            entity_path,
            heap_size_bytes: _, // use the method instead because of lazy initialization
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let mut fields: Vec<ArrowField> = vec![];
        let mut metadata = std::collections::HashMap::default();
        let mut columns: Vec<ArrowArrayRef> =
            Vec::with_capacity(1 /* row_ids */ + timelines.len() + components.len());

        // Chunk-level metadata
        {
            re_tracing::profile_scope!("metadata");

            metadata.extend(TransportChunk::chunk_metadata_id(*id));

            metadata.extend(TransportChunk::chunk_metadata_entity_path(entity_path));

            metadata.extend(TransportChunk::chunk_metadata_heap_size_bytes(
                self.heap_size_bytes(),
            ));

            if *is_sorted {
                metadata.extend(TransportChunk::chunk_metadata_is_sorted());
            }
        }

        // Row IDs
        {
            re_tracing::profile_scope!("row ids");

            fields.push(
                ArrowField::new(
                    RowId::descriptor().to_string(),
                    RowId::arrow_datatype().clone(),
                    false,
                )
                .with_metadata(
                    TransportChunk::field_metadata_control_column().tap_mut(|metadata| {
                        // This ensures the RowId/Tuid is formatted correctly
                        metadata.insert(
                            "ARROW:extension:name".to_owned(),
                            re_tuid::Tuid::ARROW_EXTENSION_NAME.to_owned(),
                        );
                    }),
                ),
            );
            columns.push(into_arrow_ref(row_ids.clone()));
        }

        // Timelines
        {
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

                    let nullable = false; // timelines within a single chunk are always dense
                    let field =
                        ArrowField::new(timeline.name().to_string(), timeline.datatype(), nullable)
                            .with_metadata({
                                let mut metadata = TransportChunk::field_metadata_time_column();
                                if *is_sorted {
                                    metadata.extend(TransportChunk::field_metadata_is_sorted());
                                }
                                metadata
                            });

                    let times = info.times_array();

                    (field, times)
                })
                .collect_vec();

            timelines
                .sort_by(|(field1, _times1), (field2, _times2)| field1.name().cmp(field2.name()));

            for (field, times) in timelines {
                fields.push(field);
                columns.push(times);
            }
        }

        // Components
        {
            re_tracing::profile_scope!("components");

            let mut components = components
                .values()
                .flat_map(|per_desc| per_desc.iter())
                .map(|(component_desc, list_array)| {
                    let list_array = ArrowListArray::from(list_array.clone());

                    let field = ArrowField::new(
                        component_desc.component_name.to_string(),
                        list_array.data_type().clone(),
                        true,
                    )
                    .with_metadata({
                        let mut metadata = TransportChunk::field_metadata_data_column();
                        metadata.extend(TransportChunk::field_metadata_component_descriptor(
                            component_desc,
                        ));
                        metadata
                    });

                    (field, as_array_ref(list_array))
                })
                .collect_vec();

            components
                .sort_by(|(field1, _data1), (field2, _data2)| field1.name().cmp(field2.name()));

            for (field, data) in components {
                fields.push(field);
                columns.push(data);
            }
        }

        let schema = ArrowSchema::new_with_metadata(fields, metadata);

        Ok(ArrowRecordBatch::try_new(schema.into(), columns)?)
    }

    /// Prepare the [`Chunk`] for transport.
    ///
    /// It is probably a good idea to sort the chunk first.
    pub fn to_transport(&self) -> ChunkResult<TransportChunk> {
        let record_batch = self.to_record_batch()?;
        Ok(TransportChunk::from(record_batch))
    }

    pub fn from_record_batch(batch: ArrowRecordBatch) -> ChunkResult<Self> {
        Self::from_transport(&TransportChunk::from(batch))
    }

    pub fn from_transport(transport: &TransportChunk) -> ChunkResult<Self> {
        re_tracing::profile_function!(format!(
            "num_columns={} num_rows={}",
            transport.num_columns(),
            transport.num_rows()
        ));

        // Metadata
        let (id, entity_path, is_sorted) = {
            re_tracing::profile_scope!("metadata");
            (
                transport.id()?,
                transport.entity_path()?,
                transport.is_sorted(),
            )
        };

        // Row IDs
        let row_ids = {
            re_tracing::profile_scope!("row ids");

            let Some(row_ids) = transport.controls().find_map(|(field, column)| {
                // TODO(cmc): disgusting, but good enough for now.
                (field.name() == RowId::descriptor().component_name.as_str()).then_some(column)
            }) else {
                return Err(ChunkError::Malformed {
                    reason: format!("missing row_id column ({:?})", transport.schema_ref()),
                });
            };

            ArrowArrayRef::from(row_ids.clone())
                .downcast_array_ref::<ArrowStructArray>()
                .ok_or_else(|| ChunkError::Malformed {
                    reason: format!(
                        "RowId data has the wrong datatype: expected {:?} but got {:?} instead",
                        RowId::arrow_datatype(),
                        *row_ids.data_type(),
                    ),
                })?
                .clone()
        };

        // Timelines
        let timelines = {
            re_tracing::profile_scope!("timelines");

            let mut timelines = IntMap::default();

            for (field, column) in transport.timelines() {
                // See also [`Timeline::datatype`]
                let timeline = match column.data_type() {
                    ArrowDatatype::Int64 => Timeline::new_sequence(field.name().as_str()),
                    ArrowDatatype::Timestamp(ArrowTimeUnit::Nanosecond, None) => {
                        Timeline::new_temporal(field.name().as_str())
                    }
                    _ => {
                        return Err(ChunkError::Malformed {
                            reason: format!(
                                "time column '{}' is not deserializable ({:?})",
                                field.name(),
                                column.data_type()
                            ),
                        });
                    }
                };

                let times = TimeColumn::read_array(&ArrowArrayRef::from(column.clone())).map_err(
                    |err| ChunkError::Malformed {
                        reason: format!("Bad time column '{}': {err}", field.name()),
                    },
                )?;

                let is_sorted = field
                    .metadata()
                    .contains_key(TransportChunk::FIELD_METADATA_MARKER_IS_SORTED_BY_TIME);

                let time_column = TimeColumn::new(is_sorted.then_some(true), timeline, times);
                if timelines.insert(timeline, time_column).is_some() {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "time column '{}' was specified more than once",
                            field.name(),
                        ),
                    });
                }
            }

            timelines
        };

        // Components
        let components = {
            let mut components = ChunkComponents::default();

            for (field, column) in transport.components() {
                let column = column
                    .downcast_array_ref::<ArrowListArray>()
                    .ok_or_else(|| ChunkError::Malformed {
                        reason: format!(
                            "The outer array in a chunked component batch must be a sparse list, got {:?}",
                            column.data_type(),
                        ),
                    })?;

                let component_desc = TransportChunk::component_descriptor_from_field(field);

                if components
                    .insert_descriptor(component_desc, column.clone())
                    .is_some()
                {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "component column '{}' was specified more than once",
                            field.name(),
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

        if let Some(heap_size_bytes) = transport.heap_size_bytes() {
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

        let transport = self.to_transport()?;
        Ok(re_log_types::ArrowMsg {
            chunk_id: re_tuid::Tuid::from_u128(self.id().as_u128()),
            timepoint_max: self.timepoint_max(),
            batch: transport.into(),
            on_release: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use nohash_hasher::IntMap;

    use re_arrow_util::arrow_util;
    use re_log_types::{
        example_components::{MyColor, MyPoint},
        Timeline,
    };

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

        for timelines in [timelines1.clone(), timelines2.clone()] {
            let chunk_original = Chunk::from_native_row_ids(
                ChunkId::new(),
                entity_path.clone(),
                None,
                &row_ids,
                timelines.clone(),
                components.clone().into_iter().collect(),
            )?;
            let mut chunk_before = chunk_original.clone();

            for _ in 0..3 {
                let chunk_in_transport = chunk_before.to_transport()?;
                let chunk_after = Chunk::from_record_batch(chunk_in_transport.clone().into())?;

                assert_eq!(
                    chunk_in_transport.entity_path()?,
                    *chunk_original.entity_path()
                );
                assert_eq!(
                    chunk_in_transport.entity_path()?,
                    *chunk_after.entity_path()
                );
                assert_eq!(
                    chunk_in_transport.heap_size_bytes(),
                    Some(chunk_after.heap_size_bytes()),
                );
                assert_eq!(
                    chunk_in_transport.num_columns(),
                    chunk_original.num_columns()
                );
                assert_eq!(chunk_in_transport.num_columns(), chunk_after.num_columns());
                assert_eq!(chunk_in_transport.num_rows(), chunk_original.num_rows());
                assert_eq!(chunk_in_transport.num_rows(), chunk_after.num_rows());

                assert_eq!(
                    chunk_in_transport.num_controls(),
                    chunk_original.num_controls()
                );
                assert_eq!(
                    chunk_in_transport.num_controls(),
                    chunk_after.num_controls()
                );
                assert_eq!(
                    chunk_in_transport.num_timelines(),
                    chunk_original.num_timelines()
                );
                assert_eq!(
                    chunk_in_transport.num_timelines(),
                    chunk_after.num_timelines()
                );
                assert_eq!(
                    chunk_in_transport.num_components(),
                    chunk_original.num_components()
                );
                assert_eq!(
                    chunk_in_transport.num_components(),
                    chunk_after.num_components()
                );

                eprintln!("{chunk_before}");
                eprintln!("{chunk_in_transport}");
                eprintln!("{chunk_after}");

                assert_eq!(chunk_before, chunk_after);

                assert!(chunk_before.are_equal(&chunk_after));

                chunk_before = chunk_after;
            }
        }

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

            let chunk_batch = chunk_before.to_chunk_batch().unwrap();

            assert_eq!(chunk_before.num_columns(), chunk_batch.num_columns());
            assert_eq!(chunk_before.num_rows(), chunk_batch.num_rows());

            let chunk_after = Chunk::from_record_batch(chunk_batch.into()).unwrap();

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
