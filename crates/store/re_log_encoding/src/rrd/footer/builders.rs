use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, BooleanArray, RecordBatch, StringArray, UInt64Array};
use arrow::datatypes::{Field, Schema, SchemaRef};
use re_chunk::{Chunk, ChunkId};
use re_log_types::{
    AbsoluteTimeRange, EntityPath, StoreId, TimeInt, TimeType, Timeline, TimelineName,
};
use re_types_core::{ComponentBatch as _, ComponentDescriptor};

use crate::{CodecError, CodecResult, RawRrdManifest};

// ---

/// Helper to build an [`RawRrdManifest`] from Rerun chunks.
#[derive(Default, Debug)]
pub struct RrdManifestBuilder {
    /// The Sorbet schema of the recording.
    sorbet_schema: re_sorbet::SchemaBuilder,

    /// Each row is a [`ChunkId`].
    column_chunk_ids: Vec<ChunkId>,

    /// Each row is a boolean indicating whether a chunk is static.
    ///
    /// Reminder: a chunk is either fully static, or fully temporal.
    column_chunk_is_static: Vec<bool>,

    // Each row carries the number of rows in the associated chunk.
    column_chunk_num_rows: Vec<u64>,

    /// Each row indicates where in the backing storage does the chunk start, in number of bytes.
    ///
    /// This _excludes_ the outer [`crate::MessageHeader`] frame.
    ///
    /// I.e. if you were to memory-map the data at `file[column_byte_offsets:column_byte_offsets+column_byte_size]`,
    /// you would end up with everything you need to decode the chunk.
    //
    // TODO(cmc): this only makes sense in the context of physical RRD files. We will need to make
    // this more generic to accommodate for other contexts (e.g. chunk keys in Redap and OSS).
    column_byte_offsets_excluding_headers: Vec<u64>,

    /// Each row indicates the size in bytes of the chunk in the backing storage, in number of bytes.
    ///
    /// This _excludes_ the outer [`crate::MessageHeader`] frame.
    ///
    /// I.e. if you were to memory-map the data at `file[column_byte_offsets:column_byte_offsets+column_byte_size]`,
    /// you would end up with everything you need to decode the chunk.
    //
    // TODO(cmc): this only makes sense in the context of physical RRD files. We will need to make
    // this more generic to accommodate for other contexts (e.g. chunk keys in Redap and OSS).
    column_byte_sizes_excluding_headers: Vec<u64>,

    /// Each row indicates the *uncompressed* size in bytes of the chunk in the backing storage, in number of bytes.
    ///
    /// This _excludes_ the outer [`crate::MessageHeader`] frame.
    column_byte_sizes_uncompressed_excluding_headers: Vec<u64>,

    /// Each row is an entity path.
    column_entity_paths: Vec<EntityPath>,

    /// A set of columns that keeps track of all static data, at the local/component level.
    columns_static: BTreeMap<ComponentDescriptor, RrdManifestIndexColumn>,

    /// A set of columns that keeps track of all temporal data, at the global/chunk level.
    columns_temporal: BTreeMap<TimelineName, RrdManifestTemporalColumn>,

    /// A set of columns that keeps track of all temporal data, at the local/component level.
    columns: BTreeMap<(TimelineName, ComponentDescriptor), RrdManifestTemporalColumn>,
}

impl RrdManifestBuilder {
    /// Appends a [`Chunk`], and therefore a new row, in the manifest.
    //
    // TODO(cmc): this only makes sense in the context of physical RRD files. We will need to make
    // this more generic to accommodate for other contexts (e.g. chunk keys in Redap and OSS).
    pub fn append(
        &mut self,
        chunk_batch: &re_sorbet::ChunkBatch,
        byte_span_excluding_header: re_span::Span<u64>,
        byte_size_uncompressed_excluding_header: u64,
    ) -> CodecResult<()> {
        self.sorbet_schema.add_chunk(chunk_batch);

        let chunk = Chunk::from_chunk_batch(chunk_batch)?;

        self.column_chunk_ids.push(chunk.id());
        self.column_chunk_is_static.push(chunk.is_static());
        self.column_chunk_num_rows.push(chunk.num_rows() as u64);
        self.column_byte_offsets_excluding_headers
            .push(byte_span_excluding_header.start);
        self.column_byte_sizes_excluding_headers
            .push(byte_span_excluding_header.len);
        self.column_byte_sizes_uncompressed_excluding_headers
            .push(byte_size_uncompressed_excluding_header);
        self.column_entity_paths.push(chunk.entity_path().clone());

        if chunk.is_static() {
            for desc in chunk.components().component_descriptors() {
                let column = self.columns_static.entry(desc.clone()).or_insert_with(|| {
                    RrdManifestIndexColumn::new_padded(
                        self.column_chunk_ids.len().saturating_sub(1),
                    )
                });

                let RrdManifestIndexColumn {
                    starts_inclusive,
                    ends_inclusive,
                    has_static_data,
                    num_rows: _, // irrelevant for static data
                } = column;

                starts_inclusive.push(TimeInt::STATIC);
                ends_inclusive.push(TimeInt::STATIC);

                // If we're here, it's necessarily `true`. Falsy values can only be
                // introduced by padding and/or temporal columns (see below).
                has_static_data.push(true);
            }

            // Not all chunks belong to all timelines -- make sure to realign all columns before
            // processing the next chunk.
            self.pad_index_columns();

            return Ok(());
        }

        #[expect(clippy::iter_over_hash_type)] // order is irrelevant
        for (timeline_name, time_column) in chunk.timelines() {
            let timeline = *time_column.timeline();

            let column = self
                .columns_temporal
                .entry(*timeline_name)
                .or_insert_with(|| RrdManifestTemporalColumn {
                    timeline,
                    index: RrdManifestIndexColumn::new_padded(
                        self.column_chunk_ids.len().saturating_sub(1),
                    ),
                });

            let RrdManifestIndexColumn {
                starts_inclusive,
                ends_inclusive,
                has_static_data,
                num_rows: _, // irrelevant for chunk-level columns, since times are always dense
            } = &mut column.index;

            let time_range = time_column.time_range();
            if time_range == AbsoluteTimeRange::EMPTY {
                starts_inclusive.push(TimeInt::STATIC);
                ends_inclusive.push(TimeInt::STATIC);
            } else {
                starts_inclusive.push(time_range.min());
                ends_inclusive.push(time_range.max());
            }

            has_static_data.push(false); // temporal chunk-level column

            for (component, time_range) in time_column.time_range_per_component(chunk.components())
            {
                let Some(component_col) = chunk.components().get(component) else {
                    return Err(crate::CodecError::ArrowDeserialization(
                        arrow::error::ArrowError::SchemaError(
                            "internally inconsistent chunk metadata, this is a bug".to_owned(),
                        ),
                    ));
                };

                let desc = &component_col.descriptor;

                let column = self
                    .columns
                    .entry((*timeline_name, desc.clone()))
                    .or_insert_with(|| RrdManifestTemporalColumn {
                        timeline,
                        index: RrdManifestIndexColumn::new_padded(
                            self.column_chunk_ids.len().saturating_sub(1),
                        ),
                    });

                let RrdManifestIndexColumn {
                    starts_inclusive,
                    ends_inclusive,
                    has_static_data,
                    num_rows,
                } = &mut column.index;

                if time_range == AbsoluteTimeRange::EMPTY {
                    starts_inclusive.push(TimeInt::STATIC);
                    ends_inclusive.push(TimeInt::STATIC);
                } else {
                    starts_inclusive.push(time_range.min());
                    ends_inclusive.push(time_range.max());
                }

                let chunk_num_rows = component_col
                    .list_array
                    .len()
                    .saturating_sub(component_col.list_array.null_count());
                num_rows.push(chunk_num_rows as u64);

                has_static_data.push(true); // temporal component-level column
            }
        }

        // Not all chunks belong to all timelines -- make sure to realign all columns before
        // processing the next chunk.
        self.pad_index_columns();

        Ok(())
    }

    pub fn build(self, store_id: StoreId) -> CodecResult<RawRrdManifest> {
        let sorbet_schema = arrow::datatypes::Schema::new_with_metadata(
            self.sorbet_schema.clone().build(),
            Default::default(),
        );
        let sorbet_schema_sha256 = RawRrdManifest::compute_sorbet_schema_sha256(&sorbet_schema)
            .map_err(CodecError::ArrowSerialization)?;

        let data = self.into_record_batch()?;

        Ok(RawRrdManifest {
            store_id,
            sorbet_schema,
            sorbet_schema_sha256,
            data,
        })
    }

    /// Pad all index columns with null values (`TimeInt::STATIC` is stored as null) until they are
    /// all the same length.
    fn pad_index_columns(&mut self) {
        let Self {
            sorbet_schema: _,
            column_chunk_ids,
            column_chunk_is_static: _, // always set, no need for padding
            column_chunk_num_rows: _,  // always set, no need for padding
            column_byte_offsets_excluding_headers: _, // always set, no need for padding
            column_byte_sizes_excluding_headers: _, // always set, no need for padding
            column_byte_sizes_uncompressed_excluding_headers: _, // always set, no need for padding
            column_entity_paths: _,    // always set, no need for padding
            columns_static,
            columns_temporal,
            columns,
        } = self;

        for (_timeline, desc) in columns.keys() {
            columns_static
                .entry(desc.clone())
                .or_insert_with(|| RrdManifestIndexColumn::new_padded(0));
        }

        for column in itertools::chain!(
            columns_static.values_mut(),
            columns_temporal.values_mut().map(|col| &mut col.index),
            columns.values_mut().map(|col| &mut col.index),
        ) {
            let num_rows_diff = column_chunk_ids
                .len()
                .saturating_sub(column.has_static_data.len());
            column.pad_extend(num_rows_diff);
        }
    }
}

impl RrdManifestBuilder {
    /// Returns the fields of the builder.
    ///
    /// The columns returned by `Self::into_columns` are guaranteed to follow these fields.
    pub fn fields(&self) -> Vec<Field> {
        itertools::chain!(
            [
                RawRrdManifest::field_chunk_entity_path(),
                RawRrdManifest::field_chunk_id(),
                RawRrdManifest::field_chunk_is_static(),
                RawRrdManifest::field_chunk_num_rows(),
            ],
            [
                RawRrdManifest::field_chunk_byte_offset(), //
                RawRrdManifest::field_chunk_byte_size(),
                RawRrdManifest::field_chunk_byte_size_uncompressed(),
            ],
            self.index_fields(),
        )
        .collect()
    }

    /// Returns the schema of the builder.
    ///
    /// The columns returned by `Self::into_columns` are guaranteed to follow this schema.
    pub fn schema(&self) -> SchemaRef {
        let fields: Vec<arrow::datatypes::Field> = self.fields().into_iter().collect();
        SchemaRef::new(Schema::new_with_metadata(fields, Default::default()))
    }

    /// Turns the builder into actual data (columns).
    ///
    /// The returned columns are guaranteed to match the schema returned by [`Self::schema`].
    #[tracing::instrument(skip_all, level = "debug")]
    pub fn into_columns(mut self) -> Vec<ArrayRef> {
        // Not all chunks belong to all timelines -- make sure to realign all columns before
        // processing the next chunk.
        self.pad_index_columns();

        let Self {
            sorbet_schema: _,
            column_chunk_ids,
            column_chunk_is_static,
            column_chunk_num_rows,
            column_byte_offsets_excluding_headers,
            column_byte_sizes_excluding_headers,
            column_byte_sizes_uncompressed_excluding_headers,
            column_entity_paths,
            columns_static,
            columns_temporal,
            columns,
        } = self;

        // FixedSizedBinaryArray(16)
        let column_chunk_ids = Arc::new(
            column_chunk_ids
                .to_arrow()
                .expect("to_arrow for ChunkIds never fails"),
        );

        let column_chunk_is_static =
            Arc::new(BooleanArray::from(column_chunk_is_static)) as ArrayRef;
        let column_chunk_num_rows = Arc::new(UInt64Array::from(column_chunk_num_rows)) as ArrayRef;

        let column_byte_offsets =
            Arc::new(UInt64Array::from(column_byte_offsets_excluding_headers)) as ArrayRef;
        let column_byte_sizes =
            Arc::new(UInt64Array::from(column_byte_sizes_excluding_headers)) as ArrayRef;
        let column_byte_sizes_uncompressed = Arc::new(UInt64Array::from(
            column_byte_sizes_uncompressed_excluding_headers,
        )) as ArrayRef;

        let column_entity_paths = Arc::new(StringArray::from_iter_values(
            column_entity_paths
                .into_iter()
                .map(|entity_path| entity_path.to_string()),
        )) as ArrayRef;

        let columns_static = columns_static
            .into_iter()
            .flat_map(|(_desc, col)| [create_index_has_data_array(col.has_static_data)]);

        let columns_temporal = columns_temporal.values().flat_map(|col| {
            [
                create_index_bound_array(col.timeline.typ(), &col.index.starts_inclusive),
                create_index_bound_array(col.timeline.typ(), &col.index.ends_inclusive),
            ]
        });

        let columns = columns.into_iter().flat_map(|(_key, col)| {
            [
                create_index_bound_array(col.timeline.typ(), &col.index.starts_inclusive),
                create_index_bound_array(col.timeline.typ(), &col.index.ends_inclusive),
                create_num_rows_array(col.index.num_rows),
            ]
        });

        [
            column_entity_paths,
            column_chunk_ids,
            column_chunk_is_static,
            column_chunk_num_rows,
            column_byte_offsets,
            column_byte_sizes,
            column_byte_sizes_uncompressed,
        ]
        .into_iter()
        .chain(columns_static)
        .chain(columns_temporal)
        .chain(columns)
        .collect()
    }

    /// Turns the builder into actual [`RecordBatch`].
    ///
    /// The returned batch are guaranteed to match the schema returned by [`Self::schema`].
    pub fn into_record_batch(self) -> CodecResult<RecordBatch> {
        let schema = self.schema();
        let num_rows = self.column_chunk_ids.len();
        let columns = self.into_columns();
        RecordBatch::try_new_with_options(
            schema,
            columns,
            &arrow::record_batch::RecordBatchOptions::new().with_row_count(Some(num_rows)),
        )
        .map_err(crate::CodecError::ArrowSerialization)
    }
}

// ---

impl RrdManifestBuilder {
    fn static_index_fields(&self) -> Vec<Field> {
        self.columns_static
            .keys()
            .flat_map(|desc| [RawRrdManifest::field_has_static_data(desc)])
            .collect()
    }

    fn temporal_index_fields(&self) -> Vec<Field> {
        self.columns_temporal
            .values()
            .flat_map(|col| {
                [
                    RawRrdManifest::field_index_start(&col.timeline, None),
                    RawRrdManifest::field_index_end(&col.timeline, None),
                ]
            })
            .collect()
    }

    fn index_fields(&self) -> Vec<Field> {
        itertools::chain!(
            self.static_index_fields(),
            self.temporal_index_fields(),
            self.columns.iter().flat_map(|((_, desc), col)| {
                [
                    RawRrdManifest::field_index_start(&col.timeline, Some(desc)),
                    RawRrdManifest::field_index_end(&col.timeline, Some(desc)),
                    RawRrdManifest::field_index_num_rows(&col.timeline, Some(desc)),
                ]
            })
        )
        .collect()
    }
}

fn create_index_bound_array(timeline_type: TimeType, times: &[TimeInt]) -> ArrayRef {
    timeline_type.make_arrow_array_from_time_ints(times.iter().copied())
}

fn create_index_has_data_array(has_data: Vec<bool>) -> ArrayRef {
    Arc::new(BooleanArray::from(has_data)) as ArrayRef
}

fn create_num_rows_array(num_rows: Vec<u64>) -> ArrayRef {
    Arc::new(UInt64Array::from(num_rows)) as ArrayRef
}

#[derive(Debug, Clone)]
struct RrdManifestTemporalColumn {
    timeline: Timeline,
    index: RrdManifestIndexColumn,
}

#[derive(Debug, Clone)]
struct RrdManifestIndexColumn {
    /// Each row contains the minimum index value found in the corresponding chunk.
    ///
    /// All values are inclusive.
    starts_inclusive: Vec<TimeInt>,

    /// Each row contains the maximum index value found in the corresponding chunk.
    ///
    /// All values are inclusive.
    ends_inclusive: Vec<TimeInt>,

    /// Each row indicates whether the corresponding chunk contains static data for the related component.
    has_static_data: Vec<bool>,

    /// Each row contains the number of rows in the corresponding chunk.
    ///
    /// This is irrelevant for chunk-level indexes, since times are always dense (i.e. the number
    /// of rows of every timeline always matches the number of rows of the chunk itself).
    num_rows: Vec<u64>,
}

impl RrdManifestIndexColumn {
    /// Instantiates a new column with `n` rows, pre-filled with default data.
    fn new_padded(n: usize) -> Self {
        Self {
            starts_inclusive: vec![TimeInt::STATIC; n],
            ends_inclusive: vec![TimeInt::STATIC; n],
            has_static_data: vec![false; n],
            num_rows: vec![0; n],
        }
    }

    /// Extends an existing column with `n` rows, filled with default data.
    fn pad_extend(&mut self, n: usize) {
        let Self {
            starts_inclusive: starts,
            ends_inclusive: ends,
            has_static_data,
            num_rows,
        } = self;

        starts.extend(std::iter::repeat_n(TimeInt::STATIC, n));
        ends.extend(std::iter::repeat_n(TimeInt::STATIC, n));
        has_static_data.extend(std::iter::repeat_n(false, n));
        num_rows.extend(std::iter::repeat_n(0, n));
    }
}
