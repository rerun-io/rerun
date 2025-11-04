use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanArray, FixedSizeBinaryArray, Int64Array, StringArray, UInt64Array,
};
use arrow::datatypes::{Field, Schema, SchemaRef};
use itertools::izip;

use re_chunk::{Chunk, ChunkId};
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, TimeType, Timeline, TimelineName};
use re_types_core::{ComponentBatch as _, ComponentDescriptor};

// ---

// TODO: there's no way we're getting away with shipping this to OSS without snapshots all over the
// place, even if we already have a gazillon on the redap side.

// TODO: we should remove these `kind` things. If Redap needs it then it should patch it in by itself.
// or maybe we just keep them, who cares...
// yeah nah im pretty sure this should go away.
//
// │ ┌──────────────────────────────────┬──────────────────────────────────┬──────────────────────────────────┬─────────────────┬───────────────────┬────────────────┐ │
// │ │ chunk_partition_id               ┆ chunk_entity_path                ┆ chunk_id                         ┆ chunk_is_static ┆ chunk_byte_offset ┆ chunk_byte_len │ │
// │ │ ---                              ┆ ---                              ┆ ---                              ┆ ---             ┆ ---               ┆ ---            │ │
// │ │ type: Utf8                       ┆ type: Utf8                       ┆ type: FixedSizeBinary[16]        ┆ type: bool      ┆ type: u64         ┆ type: u64      │ │
// │ │ kind: control                    ┆ kind: control                    ┆ kind: control                    ┆ kind: control   ┆                   ┆                │ │
// │ ╞══════════════════════════════════╪══════════════════════════════════╪══════════════════════════════════╪═════════════════╪═══════════════════╪════════════════╡ │

// TODO: im feeling more and more confident that we need encoding/decoding test cases that just
// multiplex RRD streams and check that only a single footer is left.
// That doesn't help checking that rrd compact|filter|merge|migrate|route do the right thing tho.

// TODO: "also this gets patched/extended in the proprietary server with extra info"
//
// TODO: ie the actual payload that one puts into a footer.
//
// TODO: maybe we want to drop the hash too in there? im undecided -- i can certainly think of
// applicable optimizations already tho, so probably
#[derive(Clone)]
pub struct RrdManifest {
    // TODO: hash
    //
    //
    //
    //
    /// The Sorbet schema of the recording, following the usual merging and sorting rules.
    //
    // TODO: sorbet_schema is a much better name to make it clear that this is _NOT_ the schema of
    // the thing below (which bears repeating in the doc comment btw).
    pub sorbet_schema: arrow::datatypes::Schema,

    /// The actual manifest, which catalogs every chunk in this recording.
    ///
    /// Carries all the relevant statistics to perform relevancy queries.
    pub manifest: arrow::array::RecordBatch,
}

impl RrdManifest {
    /// Returns the raw Arrow data for the partition ID column.
    pub fn col_chunk_partition_id_raw(&self) -> crate::CodecResult<&StringArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        self.manifest
            .column_by_name(CHUNK_PARTITION_ID_FIELD_NAME)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot read column: '{CHUNK_PARTITION_ID_FIELD_NAME}' is missing from batch",
                )))
            })?
            .downcast_array_ref::<StringArray>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot downcast column: '{CHUNK_PARTITION_ID_FIELD_NAME}' is not a StringArray",
                )))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the partition ID column.
    ///
    /// This is free.
    pub fn col_chunk_partition_id(&self) -> crate::CodecResult<impl Iterator<Item = &str>> {
        Ok(self.col_chunk_partition_id_raw()?.iter().flatten())
    }

    /// Returns the raw Arrow data for the entity path column.
    pub fn col_chunk_entity_path_raw(&self) -> crate::CodecResult<&StringArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        self.manifest
            .column_by_name(CHUNK_ENTITY_PATH_FIELD_NAME)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot read column: '{CHUNK_ENTITY_PATH_FIELD_NAME}' is missing from batch",
                )))
            })?
            .downcast_array_ref::<StringArray>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot downcast column: '{CHUNK_ENTITY_PATH_FIELD_NAME}' is not a StringArray",
                )))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the entity path column.
    ///
    /// This might incur interning costs, but is otherwise basically free.
    pub fn col_chunk_entity_path(&self) -> crate::CodecResult<impl Iterator<Item = EntityPath>> {
        let col_raw = self.col_chunk_entity_path_raw()?;

        Ok(col_raw.iter().flatten().map(EntityPath::parse_forgiving))
    }

    /// Returns the raw Arrow data for the chunk ID column.
    pub fn col_chunk_id_raw(&self) -> crate::CodecResult<&FixedSizeBinaryArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        self.manifest
            .column_by_name(CHUNK_ID_FIELD_NAME)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot read column: '{CHUNK_ID_FIELD_NAME}' is missing from batch",
                )))
            })?
            .downcast_array_ref::<FixedSizeBinaryArray>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot downcast column: '{CHUNK_ID_FIELD_NAME}' is not a FixedSizeBinaryArray",
                )))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the chunk ID column.
    ///
    /// This incurs a very cheap copy, but is otherwise basically free.
    pub fn col_chunk_id(&self) -> crate::CodecResult<impl Iterator<Item = ChunkId>> {
        Ok(self
            .col_chunk_id_raw()?
            .iter()
            .flatten()
            .filter_map(|bytes| {
                let bytes: [u8; 16] = bytes
                    .try_into()
                    .inspect_err(|err| {
                        tracing::error!(
                            %err,
                            ?bytes,
                            "failed to parse chunk ID from fixed-size binary array"
                        );
                    })
                    .ok()?;
                Some(ChunkId::from_tuid(Tuid::from_bytes(bytes)))
            }))
    }

    /// Returns the raw Arrow data for the is-static column.
    pub fn col_chunk_is_static_raw(&self) -> crate::CodecResult<&BooleanArray> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        self.manifest
            .column_by_name(CHUNK_IS_STATIC_FIELD_NAME)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot read column: '{CHUNK_IS_STATIC_FIELD_NAME}' is missing from batch",
                )))
            })?
            .downcast_array_ref::<BooleanArray>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot downcast column: '{CHUNK_IS_STATIC_FIELD_NAME}' is not a BooleanArray",
                )))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the is-static column.
    ///
    /// This is free.
    pub fn col_chunk_is_static(&self) -> crate::CodecResult<impl Iterator<Item = bool>> {
        Ok(self.col_chunk_is_static_raw()?.iter().flatten())
    }

    /// Returns the raw Arrow data for the byte-offset column.
    pub fn col_chunk_byte_offset_raw(&self) -> crate::CodecResult<&UInt64Array> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        self.manifest
            .column_by_name(CHUNK_BYTE_OFFSET_FIELD_NAME)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot read column: '{CHUNK_BYTE_OFFSET_FIELD_NAME}' is missing from batch",
                )))
            })?
            .downcast_array_ref::<UInt64Array>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot downcast column: '{CHUNK_BYTE_OFFSET_FIELD_NAME}' is not a UInt64Array",
                )))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the byte-offset column.
    ///
    /// This is free.
    pub fn col_chunk_byte_offset(&self) -> crate::CodecResult<impl Iterator<Item = u64>> {
        Ok(self.col_chunk_byte_offset_raw()?.iter().flatten())
    }

    /// Returns the raw Arrow data for the byte-length column.
    pub fn col_chunk_byte_len_raw(&self) -> crate::CodecResult<&UInt64Array> {
        use re_arrow_util::ArrowArrayDowncastRef as _;
        self.manifest
            .column_by_name(CHUNK_BYTE_LEN_FIELD_NAME)
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot read column: '{CHUNK_BYTE_LEN_FIELD_NAME}' is missing from batch",
                )))
            })?
            .downcast_array_ref::<UInt64Array>()
            .ok_or_else(|| {
                crate::CodecError::ArrowDeserialization(arrow::error::ArrowError::SchemaError(format!(
                    "cannot downcast column: '{CHUNK_BYTE_LEN_FIELD_NAME}' is not a UInt64Array",
                )))
            })
    }

    /// Returns an iterator over the decoded Arrow data for the byte-length column.
    ///
    /// This is free.
    pub fn col_chunk_byte_len(&self) -> crate::CodecResult<impl Iterator<Item = u64>> {
        Ok(self.col_chunk_byte_len_raw()?.iter().flatten())
    }
}

// ---

// TODO: bro all of this is such a fucking mess though
// * so much is public and im not sure why

/// Helper to build an RRD Manifest from Rerun chunks.
#[derive(Default, Debug)]
pub struct RrdManifestBuilder {
    /// Each row is a [`ChunkId`].
    column_chunk_ids: Vec<ChunkId>,

    /// Each row is a boolean indicating whether a chunk is static.
    ///
    /// Reminder: a chunk is either fully static, or fully temporal.
    column_chunk_is_static: Vec<bool>,

    /// Each row indicates where in the backing storage does the chunk start, in number of bytes.
    ///
    /// This _excludes_ the outer [`crate::MessageHeader`] frame.
    ///
    /// I.e. if you were to memory-map the data at `file[column_byte_offsets:column_byte_offsets+column_byte_len]`,
    /// you would end up with everything you need to decode the chunk.
    column_byte_offsets_excluding_headers: Vec<u64>,

    /// Each row indicates the size in bytes of the chunk in the backing storage, in number of bytes.
    ///
    /// This _excludes_ the outer [`crate::MessageHeader`] frame.
    ///
    /// I.e. if you were to memory-map the data at `file[column_byte_offsets:column_byte_offsets+column_byte_len]`,
    /// you would end up with everything you need to decode the chunk.
    column_byte_lens_excluding_headers: Vec<u64>,

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
    // TODO: clearly an actual chunk is gonna be a problem perf-wise, since the encoder gets a raw recordbatch.
    // actually not necessarily true: the most costly part is the conversion from bytes to arrow,
    // arrow to chunk (i.e. btreemaps and shit) is probably not that bad actually.
    //
    // We will go through all chunks in the recording, turning each one of them into a single
    // row in the manifest.
    // Because not every chunk will contain data for every single component nor timeline, the
    // manifest is a very sparse table.
    // We deal with that sparseness by padding columns with default values as needed.
    pub fn append(
        &mut self,
        chunk: &Chunk,
        byte_offset_excluding_header: u64,
        byte_size_excluding_header: u64,
    ) -> crate::CodecResult<()> {
        self.column_chunk_ids.push(chunk.id());
        self.column_chunk_is_static.push(chunk.is_static());
        self.column_byte_offsets_excluding_headers
            .push(byte_offset_excluding_header);
        self.column_byte_lens_excluding_headers
            .push(byte_size_excluding_header);
        self.column_entity_paths.push(chunk.entity_path().clone());

        if chunk.is_static() {
            for desc in chunk.components().component_descriptors() {
                let column = self.columns_static.entry(desc.clone()).or_insert_with(|| {
                    RrdManifestIndexColumn::new_padded(
                        self.column_chunk_ids.len().saturating_sub(1),
                    )
                });

                let RrdManifestIndexColumn {
                    starts,
                    ends,
                    has_data,
                } = column;

                starts.push(TimeInt::STATIC);
                ends.push(TimeInt::STATIC);

                // If we're here, it's necessarily `true`. Falsy values can only be
                // introduced by padding (see below).
                has_data.push(true);
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
                starts,
                ends,
                has_data,
            } = &mut column.index;

            let time_range = time_column.time_range();
            if time_range == AbsoluteTimeRange::EMPTY {
                starts.push(TimeInt::STATIC);
                ends.push(TimeInt::STATIC);
            } else {
                starts.push(time_range.min());
                ends.push(time_range.max());
            }

            has_data.push(false); // value is irrelevant, this is non-sensical for a global temporal column

            for (component, time_range) in time_column.time_range_per_component(chunk.components())
            {
                let Some(desc) = chunk.components().get_descriptor(component) else {
                    return Err(crate::CodecError::ArrowDeserialization(
                        arrow::error::ArrowError::SchemaError(
                            "internally inconsistent chunk metadata, this is a bug".to_owned(),
                        ),
                    ));
                };

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
                    starts,
                    ends,
                    has_data,
                } = &mut column.index;

                if time_range == AbsoluteTimeRange::EMPTY {
                    starts.push(TimeInt::STATIC);
                    ends.push(TimeInt::STATIC);
                } else {
                    starts.push(time_range.min());
                    ends.push(time_range.max());
                }

                // If we're here, it's necessarily `true`. Falsy values can only be
                // introduced by padding (see below).
                has_data.push(true);
            }
        }

        // Not all chunks belong to all timelines -- make sure to realign all columns before
        // processing the next chunk.
        self.pad_index_columns();

        Ok(())
    }

    /// Pad all index columns with null values (`TimeInt::STATIC` is stored as null) until they are
    /// all the same length.
    fn pad_index_columns(&mut self) {
        let Self {
            column_chunk_ids,
            column_chunk_is_static: _, // always set, no need for padding
            column_byte_offsets_excluding_headers: _, // always set, no need for padding
            column_byte_lens_excluding_headers: _, // always set, no need for padding
            column_entity_paths: _,    // always set, no need for padding
            columns_static,
            columns_temporal,
            columns,
        } = self;

        for (_timeline, desc) in columns.keys() {
            columns_static
                .entry(desc.clone())
                .or_insert(RrdManifestIndexColumn::new_padded(0));
        }

        for column in itertools::chain!(
            columns_static.values_mut(),
            columns_temporal.values_mut().map(|col| &mut col.index),
            columns.values_mut().map(|col| &mut col.index),
        ) {
            let num_rows_diff = column_chunk_ids.len().saturating_sub(column.has_data.len());
            column.pad_extend(num_rows_diff);
        }
    }

    /// Returns the schema of the builder.
    ///
    /// The columns returns by `Self::into_columns` are guaranteed to follow this schema.
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
            column_chunk_ids,
            column_chunk_is_static,
            column_byte_offsets_excluding_headers,
            column_byte_lens_excluding_headers,
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

        let column_byte_offsets =
            Arc::new(UInt64Array::from(column_byte_offsets_excluding_headers)) as ArrayRef;
        let column_byte_lens =
            Arc::new(UInt64Array::from(column_byte_lens_excluding_headers)) as ArrayRef;

        let column_entity_paths = Arc::new(StringArray::from_iter_values(
            column_entity_paths
                .into_iter()
                .map(|entity_path| entity_path.to_string()),
        )) as ArrayRef;

        let columns_static = columns_static
            .into_iter()
            .flat_map(|(_desc, col)| [create_index_has_data_array(col.has_data)]);

        let columns_temporal = columns_temporal.values().flat_map(|col| {
            [
                create_index_bound_array(col.timeline.typ(), &col.index.starts),
                create_index_bound_array(col.timeline.typ(), &col.index.ends),
                create_index_len_array(&col.index.starts, &col.index.ends),
            ]
        });

        let columns = columns.into_iter().flat_map(|(_key, col)| {
            [
                create_index_bound_array(col.timeline.typ(), &col.index.starts),
                create_index_bound_array(col.timeline.typ(), &col.index.ends),
                create_index_has_data_array(col.index.has_data),
            ]
        });

        [
            column_entity_paths,
            column_chunk_ids,
            column_chunk_is_static,
            column_byte_offsets,
            column_byte_lens,
        ]
        .into_iter()
        .chain(columns_static)
        .chain(columns_temporal)
        .chain(columns)
        .collect()
    }
}

impl RrdManifestBuilder {
    pub fn static_index_fields(&self) -> Vec<Field> {
        self.columns_static
            .keys()
            .flat_map(|desc| [Self::has_static_data_index_field(desc)])
            .collect()
    }

    pub fn temporal_index_fields(&self) -> Vec<Field> {
        self.columns_temporal
            .values()
            .flat_map(|col| {
                [
                    Self::start_index_field(&col.timeline, None),
                    Self::end_index_field(&col.timeline, None),
                    Self::length_index_field(&col.timeline, None),
                ]
            })
            .collect()
    }

    pub fn index_fields(&self) -> Vec<Field> {
        itertools::chain!(
            self.static_index_fields(),
            self.temporal_index_fields(),
            self.columns.iter().flat_map(|((_, desc), col)| {
                [
                    Self::start_index_field(&col.timeline, Some(desc)),
                    Self::end_index_field(&col.timeline, Some(desc)),
                    Self::has_data_index_field(&col.timeline, desc),
                ]
            })
        )
        .collect()
    }

    pub fn fields(&self) -> Vec<Field> {
        itertools::chain!(
            [
                Self::entity_path_field(),
                Self::chunk_id_field(),
                Self::chunk_is_static_field(),
            ],
            Self::byte_fields(),
            self.index_fields(),
        )
        .collect()
    }
}

fn create_index_bound_array(timeline_type: TimeType, times: &[TimeInt]) -> ArrayRef {
    timeline_type.make_arrow_array_from_time_ints(times.iter().copied())
}

fn create_index_len_array(starts: &[TimeInt], ends: &[TimeInt]) -> ArrayRef {
    Arc::new(
        izip!(starts, ends)
            .map(|(start, end)| {
                if start.is_static() || end.is_static() {
                    None
                } else {
                    Some(end.as_i64().saturating_sub(start.as_i64()))
                }
            })
            .collect::<Int64Array>(),
    ) as ArrayRef
}

fn create_index_has_data_array(has_data: Vec<bool>) -> ArrayRef {
    Arc::new(BooleanArray::from(has_data)) as ArrayRef
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
    starts: Vec<TimeInt>,

    /// Each row contains the maximum index value found in the corresponding chunk.
    ///
    /// All values are inclusive.
    ends: Vec<TimeInt>,

    /// Each row indicates whether the corresponding chunk actually contains the relevant data.
    has_data: Vec<bool>,
}

impl RrdManifestIndexColumn {
    /// Instantiates a new column with `n` rows, pre-filled with default data.
    fn new_padded(n: usize) -> Self {
        Self {
            starts: vec![TimeInt::STATIC; n],
            ends: vec![TimeInt::STATIC; n],
            has_data: vec![false; n],
        }
    }

    /// Extends an existing column with `n` rows, filled with default data.
    fn pad_extend(&mut self, n: usize) {
        let Self {
            starts,
            ends,
            has_data,
        } = self;

        starts.extend(std::iter::repeat_n(TimeInt::STATIC, n));
        ends.extend(std::iter::repeat_n(TimeInt::STATIC, n));
        has_data.extend(std::iter::repeat_n(false, n));
    }
}

// ---

// TODO: should we put those in the RrdManifest scope maybe?
// TODO: maybe we need some schema checks all over the place like we do for ext types?
// TODO: maybe the ToApplication impl could check that things aren't completely outta whack or something
pub const CHUNK_PARTITION_ID_FIELD_NAME: &str = "chunk_partition_id";
pub const CHUNK_ID_FIELD_NAME: &str = "chunk_id";
pub const CHUNK_IS_STATIC_FIELD_NAME: &str = "chunk_is_static";
pub const CHUNK_ENTITY_PATH_FIELD_NAME: &str = "chunk_entity_path";
pub const CHUNK_BYTE_OFFSET_FIELD_NAME: &str = "chunk_byte_offset";
pub const CHUNK_BYTE_LEN_FIELD_NAME: &str = "chunk_byte_len";

// Schema helpers

impl RrdManifestBuilder {
    pub fn byte_fields() -> Vec<Field> {
        vec![
            Self::chunk_byte_offset_field(), //
            Self::chunk_byte_len_field(),
        ]
    }

    pub fn chunk_byte_offset_field() -> Field {
        Self::byte_field(CHUNK_BYTE_OFFSET_FIELD_NAME)
    }

    pub fn chunk_byte_len_field() -> Field {
        Self::byte_field(CHUNK_BYTE_LEN_FIELD_NAME)
    }

    #[inline]
    pub fn chunk_id_field() -> Field {
        use re_log_types::external::re_types_core::Loggable as _;
        let nullable = false; // every chunk has an ID
        #[expect(clippy::iter_on_single_items)] // futureproof
        Field::new(CHUNK_ID_FIELD_NAME, ChunkId::arrow_datatype(), nullable).with_metadata(
            [
                ("rerun:kind".to_owned(), "control".to_owned()), //
            ]
            .into_iter()
            .collect(),
        )
    }

    #[inline]
    pub fn chunk_is_static_field() -> Field {
        let nullable = false; // every chunk is either static or temporal
        #[expect(clippy::iter_on_single_items)] // futureproof
        Field::new(
            CHUNK_IS_STATIC_FIELD_NAME,
            arrow::datatypes::DataType::Boolean,
            nullable,
        )
        .with_metadata(
            [
                ("rerun:kind".to_owned(), "control".to_owned()), //
            ]
            .into_iter()
            .collect(),
        )
    }

    #[inline]
    pub fn partition_id_field() -> Field {
        let nullable = false; // every chunk has an associated partition ID
        #[expect(clippy::iter_on_single_items)] // futureproof
        Field::new(
            CHUNK_PARTITION_ID_FIELD_NAME,
            arrow::datatypes::DataType::Utf8,
            nullable,
        )
        .with_metadata(
            [
                ("rerun:kind".to_owned(), "control".to_owned()), //
            ]
            .into_iter()
            .collect(),
        )
    }

    #[inline]
    pub fn entity_path_field() -> Field {
        let nullable = false; // every chunk has an entity path
        #[expect(clippy::iter_on_single_items)] // futureproof
        Field::new(
            CHUNK_ENTITY_PATH_FIELD_NAME,
            arrow::datatypes::DataType::Utf8,
            nullable,
        )
        .with_metadata(
            [
                ("rerun:kind".to_owned(), "control".to_owned()), //
            ]
            .into_iter()
            .collect(),
        )
    }

    #[inline]
    pub fn byte_field(name: &str) -> Field {
        let nullable = false; // every chunk has an offset and length
        Field::new(name, arrow::datatypes::DataType::UInt64, nullable)
    }

    // TODO(emilk, zehiko, cmc): `Timeline` should not be a thing anymore, this should be an `Index`.
    #[inline]
    pub fn index_field(
        timeline: &Timeline,
        datatype: arrow::datatypes::DataType,
        desc: Option<&ComponentDescriptor>,
        marker: &str,
    ) -> Field {
        // TODO(jleibs, david, zehiko, cmc): I would love to use a common, centralized sanitizer here,
        // but it is unclear what should happen to columns such as e.g.:
        // `stable_time__Transform3D:RotationAxisAngle#rotation_axis_angle__has_data`
        //
        // The existing sanitizer as it stands would completely deface that name, in a way that
        // would make it impossible to find your data by just copy pasting a descriptor in.
        //
        // I'm sure someone will come up with final column naming guidelines at some point, we can
        // revisit this then.
        let index_name = timeline.name();

        let field_name = lance_column_name(None, None, desc, Some(index_name), Some(marker));

        let mut metadata = std::collections::HashMap::default();
        metadata.extend([
            ("rerun:index".to_owned(), timeline.name().to_string()), //
            ("rerun:index_marker".to_owned(), marker.to_owned()),    //
            ("rerun:index_kind".to_owned(), timeline.typ().to_string()), //
        ]);
        if let Some(desc) = desc {
            metadata.extend(
                [
                    Some((
                        "rerun:component_descriptor".to_owned(),
                        desc.display_name().to_owned(),
                    )),
                    desc.component_type.map(|component_type| {
                        (
                            "rerun:component_type".to_owned(),
                            component_type.full_name().to_owned(),
                        )
                    }),
                    desc.archetype
                        .as_ref()
                        .map(|name| ("rerun:archetype".to_owned(), name.full_name().to_owned())),
                    Some(("rerun:component".to_owned(), desc.component.to_string())),
                ]
                .into_iter()
                .flatten(),
            );
        }

        let nullable = true; // A) static B) not all chunks belong to all timelines
        Field::new(field_name, datatype, nullable).with_metadata(metadata)
    }

    #[inline]
    pub fn start_index_field(timeline: &Timeline, desc: Option<&ComponentDescriptor>) -> Field {
        Self::index_field(timeline, timeline.datatype(), desc, "start")
    }

    #[inline]
    pub fn end_index_field(timeline: &Timeline, desc: Option<&ComponentDescriptor>) -> Field {
        Self::index_field(timeline, timeline.datatype(), desc, "end")
    }

    // TODO: that's interesting. clearly this shit doesn't belong here, ye?
    //
    // NOTE: We have to fully materialize this (i.e. as opposed to just do a subtraction at query time),
    // because we're gonna need a scalar index for it.
    #[inline]
    pub fn length_index_field(timeline: &Timeline, desc: Option<&ComponentDescriptor>) -> Field {
        Self::index_field(timeline, arrow::datatypes::DataType::Int64, desc, "len")
    }

    #[inline]
    pub fn has_data_index_field(timeline: &Timeline, desc: &ComponentDescriptor) -> Field {
        Self::index_field(
            timeline,
            arrow::datatypes::DataType::Boolean,
            Some(desc),
            "has_data",
        )
    }

    #[inline]
    pub fn has_static_data_index_field(desc: &ComponentDescriptor) -> Field {
        let index_name = "static";
        let field_name =
            lance_column_name(None, None, Some(desc), Some(index_name), Some("has_data"));

        let mut metadata = std::collections::HashMap::default();
        metadata.extend(
            [
                Some(("rerun:index".to_owned(), index_name.to_owned())), //
                Some((
                    "rerun:component_descriptor".to_owned(),
                    desc.display_name().to_owned(),
                )),
                desc.component_type.map(|component_type| {
                    (
                        "rerun:component_type".to_owned(),
                        component_type.full_name().to_owned(),
                    )
                }),
                desc.archetype
                    .as_ref()
                    .map(|name| ("rerun:archetype".to_owned(), name.full_name().to_owned())),
                Some(("rerun:component".to_owned(), desc.component.to_string())),
            ]
            .into_iter()
            .flatten(),
        );

        let nullable = true; // only concerns static chunks
        Field::new(field_name, arrow::datatypes::DataType::Boolean, nullable)
            .with_metadata(metadata)
    }
}

// ---

// TODO: is this actually needed tho?
// TODO: is there like a sorbet equivalent of this somewhere?

/// Returns a Lance column name from provided parts.
///
/// Column name is sanitized and safe for using as Lance dataset column name.
/// Note: if caller doesn't provide any part (i.e. all are `None`), an empty
/// string is returned.
///
/// Note that column naming is based on these 2 proposals:
/// <https://www.notion.so/rerunio/Canonical-column-identifier-for-dataframe-queries-206b24554b1980d98454eb989703ce2b>
/// <https://www.notion.so/rerunio/Canonical-column-identifier-for-properties-215b24554b1980029ff1cc6cdfad3f76>
/// Other use cases are internal column names used for chunk and partition manifests)
fn lance_column_name(
    entity_path: Option<&EntityPath>,
    strip_entity_prefix: Option<&str>,
    component_desc: Option<&ComponentDescriptor>,
    prefix: Option<&str>,
    suffix: Option<&str>,
) -> String {
    use re_types_core::reflection::ComponentDescriptorExt as _;
    let full_name = [
        prefix.map(ToOwned::to_owned),
        entity_path.map(|p| {
            let path = p.to_string();
            // Optionally strip the entity prefix if provided
            let path = strip_entity_prefix
                .and_then(|prefix| path.strip_prefix(prefix))
                .unwrap_or(&path);
            // Always strip trailing slashes (if present)
            path.strip_suffix("/").unwrap_or(path).to_owned()
        }),
        component_desc
            .and_then(|descr| descr.archetype)
            .map(|archetype| archetype.short_name().to_owned()),
        component_desc.map(|descr| descr.archetype_field_name().to_owned()),
        suffix.map(ToOwned::to_owned),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(":");

    let sanitized = sanitize_lance_column_name(&full_name);

    // Remove leading underscore if present
    sanitized.trim_start_matches('_').to_owned()
}

/// Lance doesn't allow some of the special characters in the column names.
/// This function replaces those special characters with `_`.
fn sanitize_lance_column_name(name: &str) -> String {
    name.replace([',', ' ', '-', '.', '\\'], "_")
}
