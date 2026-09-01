use std::sync::Arc;

use arrow::array::{ArrayRef, BinaryArray, RecordBatch, StringArray, UInt64Array};
use arrow::datatypes::Schema;
use arrow::error::ArrowError;
use re_chunk::external::re_byte_size;
use re_log_types::StoreId;

use super::RawRrdManifest;
use crate::{CodecError, CodecResult};

/// A [`RawRrdManifest`] extended with the columns Rerun Hub attaches when it serves a manifest.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct HubRrdManifest {
    pub store_id: StoreId,
    pub sorbet_schema: arrow::datatypes::Schema,
    pub sorbet_schema_sha256: [u8; 32],

    /// The raw manifest's columns plus the three hub columns appended in this order:
    /// `chunk_partition_id`, `rerun_partition_layer`, `chunk_key`.
    data: RecordBatch,
}

impl HubRrdManifest {
    /// The segment the chunk belongs to, repeated on every row.
    pub const COLUMN_CHUNK_PARTITION_ID: quiver::ColumnDesc<re_types_core::SegmentId> =
        quiver::ColumnDesc::new_with_metadata(
            "HubRrdManifest",
            "chunk_partition_id",
            &[("rerun:kind", "control")],
        );

    /// The layer the chunk belongs to, repeated on every row.
    pub const COLUMN_RERUN_PARTITION_LAYER: quiver::ColumnDesc<re_types_core::LayerName> =
        quiver::ColumnDesc::new("HubRrdManifest", "rerun_partition_layer");

    /// Opaque key encoding where to fetch the chunk.
    pub const COLUMN_CHUNK_KEY: quiver::ColumnDesc<quiver::Binary> =
        RawRrdManifest::COLUMN_CHUNK_KEY;

    /// The number of trailing hub columns in [`Self::data`].
    const NUM_HUB_COLUMNS: usize = 3;
}

impl HubRrdManifest {
    /// Crate an `HubRrdManifest` from a [`RawRrdManifest`].
    pub fn try_from_raw(
        raw: &RawRrdManifest,
        segment_id: &re_types_core::SegmentId,
        layer: &re_types_core::LayerName,
        storage_url: &url::Url,
        etag: Option<&re_protos::cloud::v1alpha1::ext::ETag>,
        registration_time: Option<jiff::Timestamp>,
    ) -> CodecResult<Self> {
        for name in [
            Self::COLUMN_CHUNK_PARTITION_ID.name,
            Self::COLUMN_RERUN_PARTITION_LAYER.name,
            Self::COLUMN_CHUNK_KEY.name,
        ] {
            if raw.data.schema_ref().column_with_name(name).is_some() {
                return Err(CodecError::ArrowDeserialization(ArrowError::SchemaError(
                    format!("raw RRD manifest already has a '{name}' column, cannot hub-extend it"),
                )));
            }
        }

        let (offsets, sizes) = header_inclusive_offsets_and_sizes(&raw.data)?;
        let chunk_keys =
            build_chunk_key_column(raw, &offsets, &sizes, storage_url, etag, registration_time)?;

        let num_rows = raw.data.num_rows();
        let partition_ids =
            StringArray::from_iter_values(std::iter::repeat_n(segment_id.to_string(), num_rows));
        let layers = StringArray::from_iter_values(std::iter::repeat_n(layer.as_str(), num_rows));

        let (schema, mut columns, row_count) = raw.data.clone().into_parts();
        let mut fields = schema.fields.to_vec();

        fields.push(Self::COLUMN_CHUNK_PARTITION_ID.arrow_field_ref());
        columns.push(Arc::new(partition_ids) as ArrayRef);

        fields.push(Self::COLUMN_RERUN_PARTITION_LAYER.arrow_field_ref());
        columns.push(Arc::new(layers) as ArrayRef);

        fields.push(Self::COLUMN_CHUNK_KEY.arrow_field_ref());
        columns.push(Arc::new(chunk_keys) as ArrayRef);

        let schema = Arc::new(Schema::new_with_metadata(fields, schema.metadata.clone()));
        let data = RecordBatch::try_new_with_options(
            schema,
            columns,
            &arrow::array::RecordBatchOptions::new().with_row_count(Some(row_count)),
        )
        .map_err(CodecError::ArrowSerialization)?;

        Ok(Self {
            store_id: raw.store_id.clone(),
            sorbet_schema: raw.sorbet_schema.clone(),
            sorbet_schema_sha256: raw.sorbet_schema_sha256,
            data,
        })
    }

    /// The extended Arrow batch: the raw manifest's columns plus the three hub columns.
    pub fn data(&self) -> &RecordBatch {
        &self.data
    }

    /// The raw manifest's fields and columns: everything before the trailing hub columns.
    pub fn raw_fields_and_columns(&self) -> (Vec<arrow::datatypes::FieldRef>, Vec<ArrayRef>) {
        let num_raw = self.num_raw_columns();
        (
            self.data.schema_ref().fields()[..num_raw].to_vec(),
            self.data.columns()[..num_raw].to_vec(),
        )
    }

    /// The `chunk_partition_id` column: the segment ID, repeated on every row.
    pub fn col_chunk_partition_id(&self) -> &ArrayRef {
        self.data.column(self.num_raw_columns())
    }

    /// The `rerun_partition_layer` column: the layer name, repeated on every row.
    pub fn col_rerun_partition_layer(&self) -> &ArrayRef {
        self.data.column(self.num_raw_columns() + 1)
    }

    /// The `chunk_key` column: one encoded `ChunkKey` per chunk.
    pub fn col_chunk_key(&self) -> &ArrayRef {
        self.data.column(self.num_raw_columns() + 2)
    }

    fn num_raw_columns(&self) -> usize {
        self.data.num_columns() - Self::NUM_HUB_COLUMNS
    }

    /// Byte ranges extended to cover the 16-byte RRD message header preceding each chunk payload.
    ///
    /// Hub chunk keys and the Segment Manifest address whole messages, headers included; the raw
    /// manifest columns describe the payload alone as expected by clients.
    pub fn chunk_byte_offsets_and_sizes_including_header(
        &self,
    ) -> CodecResult<(UInt64Array, UInt64Array)> {
        header_inclusive_offsets_and_sizes(&self.data)
    }

    /// The extended batch is still a valid raw manifest:
    pub fn into_raw(self) -> RawRrdManifest {
        let Self {
            store_id,
            sorbet_schema,
            sorbet_schema_sha256,
            data,
        } = self;

        RawRrdManifest {
            store_id,
            sorbet_schema,
            sorbet_schema_sha256,
            data,
        }
    }
}

/// Extends `data`'s payload-only offset/size columns to cover the RRD message header.
//
// TODO(RR-5382): confine this offsetting to the chunk scanner.
fn header_inclusive_offsets_and_sizes(
    data: &RecordBatch,
) -> CodecResult<(UInt64Array, UInt64Array)> {
    let header_size = crate::MessageHeader::ENCODED_SIZE_BYTES as u64;

    let offsets = RawRrdManifest::COLUMN_CHUNK_BYTE_OFFSET.extract(data)?;
    let offsets: UInt64Array = offsets
        .iter()
        .map(|offset| {
            offset.checked_sub(header_size).ok_or_else(|| {
                CodecError::FrameDecoding(format!(
                    "chunk byte offset {offset} is smaller than the RRD message header ({header_size} bytes)"
                ))
            })
        })
        .collect::<CodecResult<Vec<_>>>()?
        .into();

    let sizes = RawRrdManifest::COLUMN_CHUNK_BYTE_SIZE.extract(data)?;
    let sizes: UInt64Array = sizes
        .iter()
        .map(|size| {
            size.checked_add(header_size).ok_or_else(|| {
                CodecError::FrameDecoding(format!(
                    "chunk byte size {size} overflows when extended by the RRD message header ({header_size} bytes)"
                ))
            })
        })
        .collect::<CodecResult<Vec<_>>>()?
        .into();

    Ok((offsets, sizes))
}

fn build_chunk_key_column(
    raw: &RawRrdManifest,
    offsets: &UInt64Array,
    sizes: &UInt64Array,
    storage_url: &url::Url,
    etag: Option<&re_protos::cloud::v1alpha1::ext::ETag>,
    registration_time: Option<jiff::Timestamp>,
) -> CodecResult<BinaryArray> {
    use re_protos::cloud::v1alpha1::ext::{ChunkKey, DataSourceKind, RrdChunkLocation};

    let chunk_keys: Vec<Vec<u8>> = itertools::izip!(
        raw.col_chunk_id_iter()?,
        offsets.values().iter().copied(),
        sizes.values().iter().copied(),
    )
    .map(|(chunk_id, offset, length)| {
        ChunkKey {
            chunk_id,
            data_source_kind: DataSourceKind::Rrd,
            location: RrdChunkLocation {
                url: storage_url.clone(),
                byte_span: re_span::Span::from_start_len(offset, length),
            }
            .as_bytes(),
            etag: etag.cloned(),
            registration_time,
        }
        .as_bytes()
    })
    .collect();

    Ok(BinaryArray::from_iter_values(chunk_keys.iter()))
}

#[cfg(test)]
mod tests {
    use re_arrow_util::RecordBatchExt as _;
    use re_protos::cloud::v1alpha1::ext::{ChunkKey, ETag, RrdChunkLocation};
    use re_types_core::{LayerName, SegmentId};
    use std::assert_matches;

    use super::HubRrdManifest;
    use crate::rrd::footer::RawRrdManifest;
    use crate::rrd::test_util::{encode_test_rrd, make_test_chunks};
    use crate::{CodecError, MessageHeader};

    fn build_manifest() -> RawRrdManifest {
        let chunks = make_test_chunks(2);
        let (file, store_id) = encode_test_rrd(&chunks);

        let file = std::fs::File::open(file.path()).expect("temp file must be readable");
        let mut footer = futures::executor::block_on(crate::rrd::read_rrd_footer(&file))
            .expect("reading the footer of a freshly encoded RRD file cannot fail")
            .expect("a freshly encoded RRD file always has a footer");

        footer
            .manifests
            .remove(&store_id)
            .expect("the footer must contain the manifest for the recording that was just encoded")
    }

    #[test]
    fn hub_extends_the_raw_manifest_with_the_three_columns() {
        let raw = build_manifest();
        let storage_url = url::Url::parse("s3://bucket/recording.rrd").expect("valid url");
        let segment_id = SegmentId::from("my_segment");
        let layer = LayerName::base();
        let etag = ETag::new("some-etag");
        let registration_time = jiff::Timestamp::from_second(1_700_000_000).expect("valid ts");

        let hub = HubRrdManifest::try_from_raw(
            &raw,
            &segment_id,
            &layer,
            &storage_url,
            Some(&etag),
            Some(registration_time),
        )
        .expect("real chunk offsets are large enough to subtract the header");

        // Raw columns are untouched: the hub batch's leading columns are exactly the raw batch.
        let num_raw_columns = raw.data.num_columns();
        let projected = hub
            .data()
            .project(&(0..num_raw_columns).collect::<Vec<_>>())
            .expect("projecting a batch's own leading columns cannot fail");
        assert_eq!(projected, raw.data, "raw columns must be untouched");

        let partition_ids = HubRrdManifest::COLUMN_CHUNK_PARTITION_ID
            .extract(hub.data())
            .unwrap();
        for value in &partition_ids {
            assert_eq!(value, segment_id.as_str());
        }

        let layers = HubRrdManifest::COLUMN_RERUN_PARTITION_LAYER
            .extract(hub.data())
            .unwrap();
        for value in &layers {
            assert_eq!(value, layer.as_str());
        }

        let raw_offsets: Vec<u64> = raw
            .col_chunk_byte_offset_iter()
            .expect("manifest built from real chunks has this column")
            .collect();
        let raw_sizes: Vec<u64> = raw
            .col_chunk_byte_size_iter()
            .expect("manifest built from real chunks has this column")
            .collect();

        let chunk_keys = HubRrdManifest::COLUMN_CHUNK_KEY
            .extract(hub.data())
            .unwrap();

        let header_size = MessageHeader::ENCODED_SIZE_BYTES as u64;
        for (i, key_bytes) in chunk_keys.iter().enumerate() {
            let chunk_key: ChunkKey = key_bytes
                .try_into()
                .expect("chunk_key must decode to a valid ChunkKey");
            let location: RrdChunkLocation = chunk_key
                .location
                .as_slice()
                .try_into()
                .expect("location must decode to a valid RrdChunkLocation");

            assert_eq!(location.url, storage_url);
            assert_eq!(location.byte_span.start, raw_offsets[i] - header_size);
            assert_eq!(location.byte_span.len, raw_sizes[i] + header_size);
            assert_eq!(chunk_key.etag, Some(etag.clone()));
            assert_eq!(chunk_key.registration_time, Some(registration_time));
        }

        // The positional accessors agree with the by-name lookups.
        for (positional, name) in [
            (
                hub.col_chunk_partition_id(),
                HubRrdManifest::COLUMN_CHUNK_PARTITION_ID.name,
            ),
            (
                hub.col_rerun_partition_layer(),
                HubRrdManifest::COLUMN_RERUN_PARTITION_LAYER.name,
            ),
            (hub.col_chunk_key(), HubRrdManifest::COLUMN_CHUNK_KEY.name),
        ] {
            let by_name = hub.data().try_get_column(name).unwrap();
            assert_eq!(positional, by_name, "accessor for '{name}' is misaligned");
        }

        hub.into_raw()
            .sanity_check_cheap()
            .expect("hub-extended batch is a legal raw manifest via COMMON_IMPL_SPECIFIC_FIELDS");
    }

    #[test]
    fn try_from_raw_errors_on_zero_offsets() {
        let chunks = make_test_chunks(1);
        let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test");
        let raw =
            RawRrdManifest::build_in_memory_from_chunks(store_id, chunks.iter().map(AsRef::as_ref))
                .expect("building an in-memory manifest from valid chunks cannot fail");

        let storage_url = url::Url::parse("s3://bucket/recording.rrd").expect("valid url");
        let segment_id = SegmentId::from("my_segment");
        let layer = LayerName::base();

        let err = HubRrdManifest::try_from_raw(&raw, &segment_id, &layer, &storage_url, None, None)
            .expect_err(
                "an in-memory manifest starts its first chunk at offset 0, which underflows \
                 when subtracting the message header",
            );
        assert_matches!(err, CodecError::FrameDecoding(_));
    }
}
