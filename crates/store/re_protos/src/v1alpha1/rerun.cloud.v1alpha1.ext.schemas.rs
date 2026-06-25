//! Strongly-typed [`quiver::Quiver`] views of the dataframes exchanged by the cloud API.
//!
//! Each struct here describes the schema (column names, types, and metadata) of one
//! response dataframe. They are re-exported from `crate::cloud::v1alpha1::ext`.

use re_log_types::EntityPath;
use re_types_core::LayerName;

use crate::common::v1alpha1::TaskId;
use crate::common::v1alpha1::ext::SegmentId;

// --- QueryDatasetResponse ---

/// Strongly-typed view of the dataframe in [`crate::cloud::v1alpha1::QueryDatasetResponse`].
///
/// See [`crate::cloud::v1alpha1::QueryDatasetResponse`] for the column semantics.
/// The field names are the column names.
#[derive(quiver::Quiver)]
pub struct QueryDatasetDataframe {
    /// The id of the chunk ([`re_chunk::ChunkId`]).
    //
    // NOTE: these `rerun:kind` values must match `re_sorbet::metadata::RERUN_KIND` usage.
    #[quiver(metadata("rerun:kind" = "control"))]
    pub chunk_id: quiver::Column<re_chunk::ChunkId>,

    /// The segment this chunk belongs to.
    #[quiver(metadata("rerun:kind" = "control"))]
    pub chunk_segment_id: quiver::Column<SegmentId>,

    /// The layer this chunk belongs to.
    pub rerun_segment_layer: quiver::Column<LayerName>,

    /// Opaque key encoding where to fetch the chunk
    /// (see [`RrdChunkLocation`](crate::cloud::v1alpha1::ext::RrdChunkLocation)).
    pub chunk_key: quiver::Column<quiver::Binary>,

    /// The entity path of the chunk.
    #[quiver(metadata("rerun:kind" = "control"))]
    pub chunk_entity_path: quiver::Column<EntityPath>,

    /// Does this chunk hold static data?
    #[quiver(metadata("rerun:kind" = "control"))]
    pub chunk_is_static: quiver::Column<bool>,

    /// Byte length of the chunk within the source object.
    ///
    /// **Deprecated**: this is a denormalized projection of
    /// [`RrdChunkLocation`](crate::cloud::v1alpha1::ext::RrdChunkLocation),
    /// which new code decodes directly out of [`Self::chunk_key`].
    /// Still emitted (and required by old clients; see RR-2677)
    /// (as is the `chunk_byte_offset` column some servers include).
    pub chunk_byte_len: quiver::Column<u64>,

    /// Uncompressed size of the chunk, if known.
    pub chunk_byte_size_uncompressed: quiver::Column<Option<u64>>,

    /// Direct (presigned) URL for fetching the source object, if the server wants
    /// the client to fetch this row via direct HTTP Range.
    pub rerun_layer_direct_url: quiver::Column<Option<quiver::Dictionary<i32, quiver::Utf8>>>,

    /// When the direct URL expires, if any.
    pub rerun_layer_direct_url_expires_at: quiver::Column<Option<quiver::Dictionary<i32, i64>>>,

    /// Per-timeline `{timeline_name}:start` columns
    /// (see [`crate::cloud::v1alpha1::QueryDatasetResponse::field_timeline_start`]).
    #[quiver(extra_columns)]
    pub extra_columns: Vec<quiver::DynColumn>,
}

// --- QueryTasksResponse ---

/// Strongly-typed view of the dataframe in [`crate::cloud::v1alpha1::QueryTasksResponse::data`].
///
/// One row per task. The field names are the column names.
#[derive(Default, quiver::Quiver)]
pub struct QueryTasksDataframe {
    /// The unique id of the task.
    pub task_id: quiver::Column<TaskId>,

    /// The kind of task, e.g. `create_partition_manifest`.
    pub kind: quiver::Column<Option<quiver::Utf8>>,

    /// Task-specific data.
    pub data: quiver::Column<Option<quiver::Utf8>>,

    /// The execution status of the task, e.g. `pending`, `success`, or `error`.
    pub exec_status: quiver::Column<quiver::Utf8>,

    /// Any messages produced by the task, e.g. the error message if it failed.
    pub msgs: quiver::Column<Option<quiver::Utf8>>,

    /// The size of the task blob, in bytes.
    pub blob_len: quiver::Column<Option<u64>>,

    /// Who currently holds the lease on this task, if anyone.
    pub lease_owner: quiver::Column<Option<quiver::Utf8>>,

    /// When the current lease expires, if any.
    pub lease_expiration: quiver::Column<Option<quiver::TimestampNanosecond>>,

    /// How many times this task has been attempted.
    pub attempts: quiver::Column<u8>,

    /// When the task was created.
    pub creation_time: quiver::Column<Option<quiver::TimestampNanosecond>>,

    /// When the task was last updated.
    pub last_update_time: quiver::Column<Option<quiver::TimestampNanosecond>>,
}

// --- RegisterWithDatasetResponse ---

/// Strongly-typed view of the dataframe in [`crate::cloud::v1alpha1::RegisterWithDatasetResponse::data`].
///
/// One row per registered data source. The field names are the column names.
#[derive(Default, quiver::Quiver)]
pub struct RegisterWithDatasetDataframe {
    /// The id of the segment the data source was registered to.
    pub rerun_segment_id: quiver::Column<SegmentId>,

    /// The layer the data source was registered as.
    pub rerun_segment_layer: quiver::Column<LayerName>,

    /// The kind of data source, e.g. `rrd`.
    pub rerun_segment_type: quiver::Column<quiver::Utf8>,

    /// Where the data source's data is stored.
    pub rerun_storage_url: quiver::Column<quiver::Utf8>,

    /// The id of the registration task, or the sentinel for synchronous success.
    pub rerun_task_id: quiver::Column<TaskId>,
}

// --- ScanSegmentTableResponse ---

/// Strongly-typed view of the dataframe in [`crate::cloud::v1alpha1::ScanSegmentTableResponse::data`].
///
/// One row per segment; all the segment's layers are folded into the list columns.
/// The field names are the column names.
#[derive(Default, quiver::Quiver)]
pub struct ScanSegmentTableDataframe {
    /// The unique identifier of the segment.
    pub rerun_segment_id: quiver::Column<SegmentId>,

    /// Layer names for this segment, one per layer.
    ///
    /// Same length as [`Self::rerun_storage_urls`].
    pub rerun_layer_names: quiver::Column<quiver::List<LayerName>>,

    /// Storage URLs for this segment, one per layer.
    ///
    /// Same length as [`Self::rerun_layer_names`].
    pub rerun_storage_urls: quiver::Column<quiver::List<quiver::Utf8>>,

    /// Keeps track of the most recent time any layer belonging to this segment
    /// was updated in any way.
    pub rerun_last_updated_at: quiver::Column<quiver::TimestampNanosecond>,

    /// Total number of chunks for this segment.
    pub rerun_num_chunks: quiver::Column<u64>,

    /// Total size in bytes for this segment.
    pub rerun_size_bytes: quiver::Column<u64>,

    /// Any per-dataset property and index-range columns appended at runtime.
    #[quiver(extra_columns)]
    pub extra_columns: Vec<quiver::DynColumn>,
}

// --- ScanDatasetManifestResponse ---

/// Strongly-typed view of the dataframe in [`crate::cloud::v1alpha1::ScanDatasetManifestResponse::data`].
///
/// See [`crate::cloud::v1alpha1::ScanDatasetManifestResponse`] for the row semantics
/// (one row per (layer, segment) pair).
/// The field names are the column names.
#[derive(Default, quiver::Quiver)]
pub struct ScanDatasetManifestDataframe {
    /// The name of the layer.
    pub rerun_layer_name: quiver::Column<LayerName>,

    /// The segment this row belongs to.
    pub rerun_segment_id: quiver::Column<SegmentId>,

    /// Where the data of this row's source is stored.
    pub rerun_storage_url: quiver::Column<quiver::Utf8>,

    /// The kind of data source backing this row, e.g. `rrd`
    /// (see [`DataSourceKind`](crate::cloud::v1alpha1::ext::DataSourceKind)).
    pub rerun_layer_type: quiver::Column<quiver::Utf8>,

    /// Time at which this row's source was initially registered.
    pub rerun_registration_time: quiver::Column<quiver::TimestampNanosecond>,

    /// When was this row of the manifest modified last?
    pub rerun_last_updated_at: quiver::Column<quiver::TimestampNanosecond>,

    /// Total number of chunks in this row's source.
    pub rerun_num_chunks: quiver::Column<u64>,

    /// Total size in bytes of this row's source.
    pub rerun_size_bytes: quiver::Column<u64>,

    /// SHA-256 hash of the schema of this row's source.
    pub rerun_schema_sha256: quiver::Column<quiver::FixedSizeBinary<32>>,

    /// The registration status of this row's source
    /// (see [`LayerRegistrationStatus`](crate::cloud::v1alpha1::ext::LayerRegistrationStatus)).
    pub rerun_registration_status: quiver::Column<quiver::Utf8>,

    /// Any per-dataset property columns appended at runtime.
    #[quiver(extra_columns)]
    pub extra_columns: Vec<quiver::DynColumn>,
}

impl ScanDatasetManifestDataframe {
    /// One row per (layer, segment) pair; all columns must have the same length.
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        layer_names: Vec<LayerName>,
        segment_ids: Vec<SegmentId>,
        storage_urls: Vec<String>,
        layer_types: Vec<String>,
        registration_times: Vec<i64>,
        last_updated_at_times: Vec<i64>,
        num_chunks: Vec<u64>,
        size_bytes: Vec<u64>,
        schema_sha256s: Vec<[u8; 32]>,
        registration_statuses: Vec<String>,
    ) -> Self {
        Self {
            rerun_layer_name: layer_names.into(),
            rerun_segment_id: segment_ids.into(),
            rerun_storage_url: storage_urls.into(),
            rerun_layer_type: layer_types.into(),
            rerun_registration_time: registration_times.into(),
            rerun_last_updated_at: last_updated_at_times.into(),
            rerun_num_chunks: num_chunks.into(),
            rerun_size_bytes: size_bytes.into(),
            rerun_schema_sha256: schema_sha256s.into(),
            rerun_registration_status: registration_statuses.into(),
            extra_columns: vec![],
        }
    }
}
