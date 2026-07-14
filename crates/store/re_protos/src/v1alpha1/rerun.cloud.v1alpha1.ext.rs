use std::sync::Arc;

use arrow::array::{Array, ArrayRef, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, FieldRef, Schema};
use arrow::error::ArrowError;
use itertools::Itertools as _;
use prost::Name as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::TimelineName;
use re_log_types::AbsoluteTimeRange;
use re_log_types::{EntityPath, EntryId, TimeInt};
use re_types_core::LayerName;

use crate::cloud::v1alpha1::ext::{QueryDatasetDataframe, ScanSegmentTableDataframe};
use crate::cloud::v1alpha1::{
    DoBandwidthTestResponse, EntryKind, FetchChunksRequest, GetDatasetSchemaResponse,
    QueryDatasetResponse, QueryTasksResponse, ScanDatasetManifestRequest,
    ScanDatasetManifestResponse, ScanSegmentTableRequest, ScanSegmentTableResponse,
    UnregisterFromDatasetResponse,
};
use crate::common::v1alpha1::ext as common_ext;
use crate::common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, SegmentId};
use crate::common::v1alpha1::{DataframePart, TaskId};
use crate::{TypeConversionError, invalid_field, missing_field};

/// Helper to simplify writing `field_XXX() -> FieldRef` methods.
macro_rules! lazy_field_ref {
    ($fld:expr) => {{
        static FIELD: std::sync::OnceLock<FieldRef> = std::sync::OnceLock::new();
        let field = FIELD.get_or_init(|| Arc::new($fld));
        Arc::clone(field)
    }};
}

// --- SegmentRegistrationStatus ---

/// Registration status for a segment/layer in the dataset manifest.
//
// TODO(cmc): not the greatest name I guess... (rename in follow up?)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerRegistrationStatus {
    /// Registration for this layer has started, i.e. the synchronous phase is over.
    Pending = 0,

    /// Registration for this layer has completed successfully.
    Done = 1,

    /// Registration for this layer has failed.
    Error = 2,

    /// This layer has been removed.
    Deleted = 3,
}

impl LayerRegistrationStatus {
    const PENDING_STR: &str = "pending";
    const DONE_STR: &str = "done";
    const ERROR_STR: &str = "error";
    const DELETED_STR: &str = "deleted";

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => Self::PENDING_STR,
            Self::Done => Self::DONE_STR,
            Self::Error => Self::ERROR_STR,
            Self::Deleted => Self::DELETED_STR,
        }
    }
}

impl std::fmt::Display for LayerRegistrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for LayerRegistrationStatus {
    type Err = crate::TypeConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Self::PENDING_STR => Ok(Self::Pending),
            Self::DONE_STR => Ok(Self::Done),
            Self::ERROR_STR => Ok(Self::Error),
            Self::DELETED_STR => Ok(Self::Deleted),
            _ => Err(crate::TypeConversionError::InvalidField {
                package_name: "rerun.cloud.v1alpha1",
                type_name: "SegmentRegistrationStatus",
                field_name: "value",
                reason: format!("invalid registration status: {s}"),
            }),
        }
    }
}

impl TryFrom<u8> for LayerRegistrationStatus {
    type Error = crate::TypeConversionError;

    fn try_from(value: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match value {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Done),
            2 => Ok(Self::Error),
            3 => Ok(Self::Deleted),
            _ => Err(crate::TypeConversionError::InvalidField {
                package_name: "rerun.cloud.v1alpha1",
                type_name: "SegmentRegistrationStatus",
                field_name: "value",
                reason: format!("invalid registration status: {value}"),
            }),
        }
    }
}

// --- RegisterWithDatasetRequest ---

#[derive(Debug, Clone)]
pub struct RegisterWithDatasetRequest {
    pub data_sources: Vec<DataSource>,
    pub on_duplicate: IfDuplicateBehavior,
}

impl TryFrom<crate::cloud::v1alpha1::RegisterWithDatasetRequest> for RegisterWithDatasetRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::RegisterWithDatasetRequest,
    ) -> Result<Self, Self::Error> {
        let crate::cloud::v1alpha1::RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        } = value;

        Ok(Self {
            data_sources: data_sources
                .into_iter()
                .map(TryInto::try_into)
                .try_collect()?,
            on_duplicate: on_duplicate.try_into()?,
        })
    }
}

impl From<RegisterWithDatasetRequest> for crate::cloud::v1alpha1::RegisterWithDatasetRequest {
    fn from(value: RegisterWithDatasetRequest) -> Self {
        Self {
            data_sources: value.data_sources.into_iter().map(Into::into).collect(),
            on_duplicate: crate::common::v1alpha1::IfDuplicateBehavior::from(value.on_duplicate)
                as i32,
        }
    }
}

// --- UnregisterFromDatasetRequest ---

#[derive(Debug)]
pub struct UnregisterFromDatasetRequest {
    pub segments_to_drop: Vec<SegmentId>,
    pub layers_to_drop: Vec<LayerName>,
    pub force: bool,
}

impl TryFrom<crate::cloud::v1alpha1::UnregisterFromDatasetRequest>
    for UnregisterFromDatasetRequest
{
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::UnregisterFromDatasetRequest,
    ) -> Result<Self, Self::Error> {
        let crate::cloud::v1alpha1::UnregisterFromDatasetRequest {
            segments_to_drop,
            layers_to_drop,
            force,
        } = value;

        Ok(Self {
            segments_to_drop: segments_to_drop
                .into_iter()
                .map(TryInto::try_into)
                .try_collect()?,
            layers_to_drop: layers_to_drop.into_iter().map(LayerName::from).collect(),
            force,
        })
    }
}

impl From<UnregisterFromDatasetRequest> for crate::cloud::v1alpha1::UnregisterFromDatasetRequest {
    fn from(value: UnregisterFromDatasetRequest) -> Self {
        Self {
            segments_to_drop: value.segments_to_drop.into_iter().map(Into::into).collect(),
            layers_to_drop: value.layers_to_drop.into_iter().map(Into::into).collect(),
            force: value.force,
        }
    }
}

impl crate::cloud::v1alpha1::UnregisterFromDatasetRequest {
    pub fn sanity_check(&self) -> tonic::Result<()> {
        let Self {
            segments_to_drop,
            layers_to_drop,
            force: _,
        } = self;

        if segments_to_drop.is_empty() && layers_to_drop.is_empty() {
            return Err(tonic::Status::invalid_argument(
                "must specify at least 1 segment ID or layer for removal",
            ));
        }

        Ok(())
    }
}

// --- QueryDatasetRequest ---

#[derive(Debug, Clone)]
pub struct QueryDatasetRequest {
    pub segment_ids: Vec<common_ext::SegmentId>,
    pub generate_direct_urls: bool,
    pub chunk_ids: Vec<re_chunk::ChunkId>,
    pub entity_paths: Vec<EntityPath>,
    pub select_all_entity_paths: bool,
    pub fuzzy_descriptors: Vec<String>,
    pub exclude_static_data: bool,
    pub exclude_temporal_data: bool,
    pub scan_parameters: Option<common_ext::ScanParameters>,
    pub query: Option<Query>,
}

impl Default for QueryDatasetRequest {
    fn default() -> Self {
        Self {
            segment_ids: vec![],
            chunk_ids: vec![],
            entity_paths: vec![],
            select_all_entity_paths: true,
            fuzzy_descriptors: vec![],
            exclude_static_data: false,
            exclude_temporal_data: false,
            scan_parameters: None,
            query: None,
            generate_direct_urls: false,
        }
    }
}

impl From<QueryDatasetRequest> for crate::cloud::v1alpha1::QueryDatasetRequest {
    fn from(value: QueryDatasetRequest) -> Self {
        Self {
            segment_ids: value.segment_ids.into_iter().map(Into::into).collect(),
            chunk_ids: value
                .chunk_ids
                .into_iter()
                .map(|chunk_id| chunk_id.as_tuid().into())
                .collect(),
            entity_paths: value.entity_paths.into_iter().map(Into::into).collect(),
            select_all_entity_paths: value.select_all_entity_paths,
            fuzzy_descriptors: value.fuzzy_descriptors,
            exclude_static_data: value.exclude_static_data,
            exclude_temporal_data: value.exclude_temporal_data,
            scan_parameters: value.scan_parameters.map(Into::into),
            query: value.query.map(Into::into),
            generate_direct_urls: value.generate_direct_urls,
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::QueryDatasetRequest> for QueryDatasetRequest {
    type Error = tonic::Status;

    fn try_from(value: crate::cloud::v1alpha1::QueryDatasetRequest) -> Result<Self, Self::Error> {
        // Support both segment_ids (new) and partition_ids (deprecated) for backward compatibility
        let segment_ids = value
            .segment_ids
            .into_iter()
            .map(TryInto::try_into)
            .try_collect()?;

        let result = Self {
            segment_ids,

            chunk_ids: value
                .chunk_ids
                .into_iter()
                .map(|tuid| {
                    let id: re_tuid::Tuid = tuid.try_into()?;
                    Ok::<_, tonic::Status>(re_chunk::ChunkId::from_u128(id.as_u128()))
                })
                .try_collect()?,

            entity_paths: value
                .entity_paths
                .into_iter()
                .map(|path| {
                    path.try_into().map_err(|err| {
                        tonic::Status::invalid_argument(format!("invalid entity path: {err}"))
                    })
                })
                .try_collect()?,

            select_all_entity_paths: value.select_all_entity_paths,

            fuzzy_descriptors: value.fuzzy_descriptors,

            exclude_static_data: value.exclude_static_data,
            exclude_temporal_data: value.exclude_temporal_data,

            scan_parameters: value
                .scan_parameters
                .map(|params| params.try_into())
                .transpose()?,

            query: value.query.map(|q| q.try_into()).transpose()?,

            generate_direct_urls: value.generate_direct_urls,
        };

        if let Some(query) = result.query.as_ref()
            && let Some(la) = query.latest_at.as_ref()
            && !la.per_segment_values.is_empty()
        {
            // Per `cloud.proto`: `per_segment_values` is mutually exclusive
            // with both `latest_at.at` and `query.range`, and requires
            // `latest_at.index` to be set. The server reconstructs global
            // bounds internally from the per-segment values, so a
            // caller-supplied `at` or `range` would be ambiguous, and a
            // missing `index` would leave the per-segment values without a
            // timeline to apply to. Without these checks, the two backends
            // diverged: dataplatform errored on missing `index` while OSS
            // silently degraded to a static-only fallback (and OSS kept
            // `range` while dataplatform overwrote it) — same illegal
            // request, backend-divergent behavior.
            if la.index.is_none() {
                return Err(tonic::Status::invalid_argument(
                    "`latest_at.index` must be set when `per_segment_values` is non-empty",
                ));
            }
            if la.at != re_log_types::TimeInt::STATIC {
                return Err(tonic::Status::invalid_argument(
                    "`latest_at.at` must be unset when `per_segment_values` is non-empty",
                ));
            }
            if query.range.is_some() {
                return Err(tonic::Status::invalid_argument(
                    "`query.range` must be unset when `per_segment_values` is non-empty",
                ));
            }
            if result.segment_ids.is_empty() {
                return Err(tonic::Status::invalid_argument(
                    "`per_segment_values` requires `segment_ids` to be non-empty",
                ));
            }
            if la.per_segment_values.len() != result.segment_ids.len() {
                return Err(tonic::Status::invalid_argument(format!(
                    "`per_segment_values.len()` ({}) must equal `segment_ids.len()` ({})",
                    la.per_segment_values.len(),
                    result.segment_ids.len(),
                )));
            }
            let mut seen = std::collections::HashSet::with_capacity(result.segment_ids.len());
            for sid in &result.segment_ids {
                if !seen.insert(sid) {
                    return Err(tonic::Status::invalid_argument(
                        "`segment_ids` must contain no duplicates when `per_segment_values` is set",
                    ));
                }
            }
        }

        Ok(result)
    }
}

// --- QueryDatasetResponse ---

impl QueryDatasetResponse {
    /// Per-timeline `{timeline_name}:start` column that carries `time_min` for each chunk.
    ///
    /// Consumed by the client's `build_segment_manifests` to compute the per-segment safe
    /// horizon; no other downstream consumer reads the time range, so there is no matching
    /// `:end` column.
    ///
    /// The column type is `Int64` because all rerun time types store `i64` internally.
    pub fn field_timeline_start(timeline_name: &str) -> FieldRef {
        let metadata = std::collections::HashMap::from([
            ("rerun:index".to_owned(), timeline_name.to_owned()),
            (
                re_sorbet::metadata::RERUN_KIND.to_owned(),
                "index".to_owned(),
            ),
            ("rerun:index_marker".to_owned(), "start".to_owned()),
        ]);
        Arc::new(
            Field::new(format!("{timeline_name}:start"), DataType::Int64, true)
                .with_metadata(metadata),
        )
    }

    pub fn create_dataframe(
        chunk_ids: Vec<re_chunk::ChunkId>,
        chunk_segment_ids: Vec<SegmentId>,
        chunk_layer_names: Vec<LayerName>,
        chunk_keys: Vec<&[u8]>,
        chunk_entity_paths: Vec<EntityPath>,
        chunk_is_static: Vec<bool>,
        chunk_byte_lengths: Vec<u64>,
        chunk_byte_lengths_uncompressed: Vec<Option<u64>>,
        chunk_direct_urls: Vec<Option<String>>,
        chunk_direct_urls_expiry: Vec<Option<i64>>,
    ) -> arrow::error::Result<RecordBatch> {
        Self::create_dataframe_with_timelines(
            chunk_ids,
            chunk_segment_ids,
            chunk_layer_names,
            chunk_keys,
            chunk_entity_paths,
            chunk_is_static,
            chunk_byte_lengths,
            chunk_byte_lengths_uncompressed,
            chunk_direct_urls,
            chunk_direct_urls_expiry,
            &Default::default(),
        )
    }

    pub fn create_dataframe_with_timelines(
        chunk_ids: Vec<re_chunk::ChunkId>,
        chunk_segment_ids: Vec<SegmentId>,
        chunk_layer_names: Vec<LayerName>,
        chunk_keys: Vec<&[u8]>,
        chunk_entity_paths: Vec<EntityPath>,
        chunk_is_static: Vec<bool>,
        chunk_byte_lengths: Vec<u64>,
        chunk_byte_lengths_uncompressed: Vec<Option<u64>>,
        chunk_direct_urls: Vec<Option<String>>,
        chunk_direct_urls_expiry: Vec<Option<i64>>,
        timelines: &std::collections::BTreeMap<String, (DataType, Vec<Option<i64>>)>,
    ) -> arrow::error::Result<RecordBatch> {
        QueryDatasetDataframe {
            chunk_id: chunk_ids.into(),
            chunk_segment_id: chunk_segment_ids.into(),
            rerun_segment_layer: chunk_layer_names.into(),
            chunk_key: chunk_keys.into(),
            chunk_entity_path: chunk_entity_paths.into(),
            chunk_is_static: chunk_is_static.into(),
            chunk_byte_len: chunk_byte_lengths.into(),
            chunk_byte_size_uncompressed: chunk_byte_lengths_uncompressed.into(),
            rerun_layer_direct_url: quiver::Column::try_from_values(chunk_direct_urls)?,
            rerun_layer_direct_url_expires_at: quiver::Column::try_from_values(
                chunk_direct_urls_expiry,
            )?,

            // Caller is responsible for producing the same `timelines` set for every response of a
            // single query, so all batches share a schema and the client can concatenate them.
            extra_columns: timelines
                .iter()
                .map(|(timeline_name, (_data_type, mins))| quiver::DynColumn {
                    field: Self::field_timeline_start(timeline_name),
                    array: Arc::new(Int64Array::from(mins.clone())),
                })
                .collect(),
        }
        .into_record_batch()
        .map_err(|err| ArrowError::InvalidArgumentError(err.to_string()))
    }
}

impl ScanSegmentTableRequest {
    /// Request every segment-table column with no filter hint.
    pub fn all() -> Self {
        Self {
            columns: Vec::new(),
            sql_filter: String::new(),
        }
    }

    /// Request selected segment-table columns with no filter hint.
    pub fn with_columns(columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            columns: columns.into_iter().map(Into::into).collect(),
            sql_filter: String::new(),
        }
    }
}

impl ScanDatasetManifestRequest {
    /// Request every dataset-manifest column with no filter hint.
    pub fn all() -> Self {
        Self {
            columns: Vec::new(),
            sql_filter: String::new(),
        }
    }

    /// Request selected dataset-manifest columns with no filter hint.
    pub fn with_columns(columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            columns: columns.into_iter().map(Into::into).collect(),
            sql_filter: String::new(),
        }
    }
}

impl FetchChunksRequest {
    // This is the only required column in the request.
    pub const FIELD_CHUNK_KEY: &str = QueryDatasetDataframe::COLUMN_CHUNK_KEY_NAME;

    //TODO(RR-2677): actually, these are also required for now.
    pub const FIELD_CHUNK_ID: &str = QueryDatasetDataframe::COLUMN_CHUNK_ID_NAME;
    pub const FIELD_CHUNK_SEGMENT_ID: &str = QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID_NAME;
    pub const FIELD_CHUNK_LAYER_NAME: &str = QueryDatasetDataframe::COLUMN_RERUN_SEGMENT_LAYER_NAME;
    pub const FIELD_CHUNK_BYTE_LENGTH: &str = QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN_NAME;

    pub fn required_column_names() -> Vec<String> {
        vec![
            Self::FIELD_CHUNK_KEY.to_owned(),
            //TODO(RR-2677): remove these
            Self::FIELD_CHUNK_ID.to_owned(),
            Self::FIELD_CHUNK_SEGMENT_ID.to_owned(),
            Self::FIELD_CHUNK_LAYER_NAME.to_owned(),
            Self::FIELD_CHUNK_BYTE_LENGTH.to_owned(),
        ]
    }
}

// --- DoMaintenanceRequest ---

#[derive(Debug, Clone)]
pub struct DoMaintenanceRequest {
    pub optimize_indexes: bool,
    pub retrain_indexes: bool,
    pub compact_fragments: bool,
    pub cleanup_before: Option<jiff::Timestamp>,
    pub unsafe_allow_recent_cleanup: bool,
}

impl TryFrom<crate::cloud::v1alpha1::DoMaintenanceRequest> for DoMaintenanceRequest {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::DoMaintenanceRequest) -> Result<Self, Self::Error> {
        let cleanup_before = value
            .cleanup_before
            .map(|ts| jiff::Timestamp::new(ts.seconds, ts.nanos))
            .transpose()?;

        Ok(Self {
            optimize_indexes: value.optimize_indexes,
            retrain_indexes: value.retrain_indexes,
            compact_fragments: value.compact_fragments,
            cleanup_before,
            unsafe_allow_recent_cleanup: value.unsafe_allow_recent_cleanup,
        })
    }
}

impl From<DoMaintenanceRequest> for crate::cloud::v1alpha1::DoMaintenanceRequest {
    fn from(value: DoMaintenanceRequest) -> Self {
        Self {
            optimize_indexes: value.optimize_indexes,
            retrain_indexes: value.retrain_indexes,
            compact_fragments: value.compact_fragments,
            cleanup_before: value.cleanup_before.map(|ts| prost_types::Timestamp {
                seconds: ts.as_second(),
                nanos: ts.subsec_nanosecond(),
            }),
            unsafe_allow_recent_cleanup: value.unsafe_allow_recent_cleanup,
        }
    }
}

// --- Bandwidth test ---

/// Default chunk size used when streaming `DoBandwidthTest` responses.
pub const BANDWIDTH_TEST_CHUNK_BYTES: u64 = 1024 * 1024;

/// Hard upper bound on `DoBandwidthTestRequest.num_bytes`.
///
/// The endpoint is purely diagnostic, so there is no legitimate reason to ask the server for
/// more than this. Larger requests are rejected with `InvalidArgument` to prevent abuse
/// (server egress / CPU / long-lived streams).
pub const MAX_BANDWIDTH_TEST_BYTES: u64 = 32 * 1024 * 1024;

/// Iterator that yields `DoBandwidthTestResponse` chunks of pseudo-random,
/// incompressible bytes (xorshift64*), summing to exactly `num_bytes`.
///
/// Used by both the local `re_server` and the production `redap_frontend` to
/// implement the `DoBandwidthTest` RPC.
pub struct BandwidthTestPayloadIter {
    remaining: u64,
    chunk_size: u64,
    state: u64,
}

impl BandwidthTestPayloadIter {
    pub fn new(num_bytes: u64) -> Self {
        Self {
            remaining: num_bytes,
            chunk_size: BANDWIDTH_TEST_CHUNK_BYTES,
            state: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

impl Iterator for BandwidthTestPayloadIter {
    type Item = DoBandwidthTestResponse;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let chunk_len = self.remaining.min(self.chunk_size) as usize;
        let mut buf = vec![0u8; chunk_len];
        let mut i = 0;
        while i < chunk_len {
            let mut x = self.state;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.state = x;
            let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes();
            let n = (chunk_len - i).min(8);
            buf[i..i + n].copy_from_slice(&v[..n]);
            i += n;
        }
        self.remaining -= chunk_len as u64;
        Some(DoBandwidthTestResponse {
            payload: buf.into(),
        })
    }
}

// --- Tasks ---

impl QueryTasksResponse {
    pub fn dataframe_part(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(QueryTasksResponse, "data"))?)
    }
}

// --- EntryFilter ---

impl crate::cloud::v1alpha1::EntryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(mut self, id: EntryId) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_entry_kind(mut self, kind: EntryKind) -> Self {
        self.entry_kind = Some(kind as i32);
        self
    }
}

pub use crate::EntryName;

// --- EntryDetails ---

#[derive(Debug, Clone)]
pub struct EntryDetails {
    pub id: re_log_types::EntryId,
    pub name: EntryName,
    pub kind: crate::cloud::v1alpha1::EntryKind,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}

impl TryFrom<crate::cloud::v1alpha1::EntryDetails> for EntryDetails {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::EntryDetails) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(crate::cloud::v1alpha1::EntryDetails, "id"))?
                .try_into()?,
            name: EntryName::new(
                value
                    .name
                    .ok_or(missing_field!(crate::cloud::v1alpha1::EntryDetails, "name"))?,
            )?,
            kind: value.entry_kind.try_into()?,
            created_at: {
                let ts = value.created_at.ok_or(missing_field!(
                    crate::cloud::v1alpha1::EntryDetails,
                    "created_at"
                ))?;
                jiff::Timestamp::new(ts.seconds, ts.nanos)?
            },
            updated_at: {
                let ts = value.updated_at.ok_or(missing_field!(
                    crate::cloud::v1alpha1::EntryDetails,
                    "updated_at"
                ))?;
                jiff::Timestamp::new(ts.seconds, ts.nanos)?
            },
        })
    }
}

impl From<EntryDetails> for crate::cloud::v1alpha1::EntryDetails {
    fn from(value: EntryDetails) -> Self {
        Self {
            id: Some(value.id.into()),
            name: Some(value.name.to_string()),
            entry_kind: value.kind as _,
            created_at: {
                let ts = value.created_at;
                Some(prost_types::Timestamp {
                    seconds: ts.as_second(),
                    nanos: ts.subsec_nanosecond(),
                })
            },
            updated_at: {
                let ts = value.updated_at;
                Some(prost_types::Timestamp {
                    seconds: ts.as_second(),
                    nanos: ts.subsec_nanosecond(),
                })
            },
        }
    }
}

// --- WatchEventsRequest ---

impl crate::cloud::v1alpha1::EventKind {
    /// Subscribe to all catalog entry lifecycle events.
    pub fn entry() -> Self {
        use crate::cloud::v1alpha1::{EntryEvents, event_kind::Kind};
        Self {
            kind: Some(Kind::Entry(EntryEvents {})),
        }
    }
}

impl crate::cloud::v1alpha1::watch_events_response::Kind {
    /// Is this a catalog entry lifecycle event (created or deleted)?
    pub fn is_entry_kind(&self) -> bool {
        use crate::cloud::v1alpha1::watch_events_response::Kind;
        match self {
            Kind::EntryCreated(_) | Kind::EntryDeleted(_) => true,
        }
    }
}

// --- WatchEventsResponse ---

/// A catalog lifecycle event delivered over the `WatchEvents` stream.
//
// TODO(RR-4853): add register events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchEventsResponse {
    EntryCreated(EntryId),
    EntryDeleted(EntryId),
}

impl TryFrom<crate::cloud::v1alpha1::WatchEventsResponse> for WatchEventsResponse {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::WatchEventsResponse) -> Result<Self, Self::Error> {
        use crate::cloud::v1alpha1::watch_events_response::Kind;

        match value.kind.ok_or(missing_field!(
            crate::cloud::v1alpha1::WatchEventsResponse,
            "kind"
        ))? {
            Kind::EntryCreated(event) => Ok(Self::EntryCreated(
                event
                    .id
                    .ok_or(missing_field!(
                        crate::cloud::v1alpha1::EntryCreatedEvent,
                        "id"
                    ))?
                    .try_into()?,
            )),
            Kind::EntryDeleted(event) => Ok(Self::EntryDeleted(
                event
                    .id
                    .ok_or(missing_field!(
                        crate::cloud::v1alpha1::EntryDeletedEvent,
                        "id"
                    ))?
                    .try_into()?,
            )),
        }
    }
}

impl From<WatchEventsResponse> for crate::cloud::v1alpha1::WatchEventsResponse {
    fn from(value: WatchEventsResponse) -> Self {
        use crate::cloud::v1alpha1::watch_events_response::Kind;
        use crate::cloud::v1alpha1::{EntryCreatedEvent, EntryDeletedEvent};

        let kind = match value {
            WatchEventsResponse::EntryCreated(id) => Kind::EntryCreated(EntryCreatedEvent {
                id: Some(id.into()),
            }),
            WatchEventsResponse::EntryDeleted(id) => Kind::EntryDeleted(EntryDeletedEvent {
                id: Some(id.into()),
            }),
        };
        Self { kind: Some(kind) }
    }
}

// --- DatasetDetails / TableDetails validation ---

/// Error returned when the blueprint configuration in [`DatasetDetails`] or [`TableDetails`] is
/// internally inconsistent.
///
/// This only covers checks that can be made without consulting the store. Callers with store
/// access must additionally verify that the referenced `blueprint_dataset` exists and is itself a
/// blueprint dataset.
#[derive(Debug, thiserror::Error)]
#[error("default {entry_kind} blueprint requires a blueprint dataset")]
pub struct InconsistentBlueprintDetailsError {
    entry_kind: &'static str,
}

// --- DatasetDetails ---

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatasetDetails {
    pub blueprint_dataset: Option<EntryId>,
    pub asset_dataset: Option<EntryId>,
    pub default_blueprint_segment: Option<SegmentId>,
    pub default_segment_table_blueprint_segment: Option<SegmentId>,
}

impl DatasetDetails {
    /// Checks that the blueprint configuration is internally consistent.
    ///
    /// A default blueprint (for either the dataset or its segment table) can only be set if a
    /// [`Self::blueprint_dataset`] is set too.
    ///
    /// This does *not* check that the blueprint dataset actually exists or is itself a blueprint
    /// dataset — callers with store access must verify that separately.
    pub fn validate_consistency(&self) -> Result<(), InconsistentBlueprintDetailsError> {
        let has_default_blueprint = self.default_blueprint_segment.is_some()
            || self.default_segment_table_blueprint_segment.is_some();
        if has_default_blueprint && self.blueprint_dataset.is_none() {
            return Err(InconsistentBlueprintDetailsError {
                entry_kind: "dataset",
            });
        }
        Ok(())
    }

    /// Returns the default blueprint for this dataset.
    ///
    /// Both `blueprint_dataset` and `default_blueprint_segment` must be set.
    pub fn default_blueprint(&self) -> Option<(EntryId, SegmentId)> {
        let blueprint = self.blueprint_dataset.as_ref()?;
        self.default_blueprint_segment
            .as_ref()
            .map(|default| (blueprint.clone(), default.clone()))
    }

    /// Returns the default blueprint for this dataset's segment table.
    ///
    /// Both `blueprint_dataset` and `default_segment_table_blueprint_segment` must be set.
    pub fn default_segment_table_blueprint(&self) -> Option<(EntryId, SegmentId)> {
        let blueprint = self.blueprint_dataset.as_ref()?;
        self.default_segment_table_blueprint_segment
            .as_ref()
            .map(|default| (blueprint.clone(), default.clone()))
    }
}

impl TryFrom<crate::cloud::v1alpha1::DatasetDetails> for DatasetDetails {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::DatasetDetails) -> Result<Self, Self::Error> {
        let default_blueprint_segment = value
            .default_blueprint_segment
            .map(TryInto::try_into)
            .transpose()?;

        let default_segment_table_blueprint_segment = value
            .default_segment_table_blueprint_segment
            .map(TryInto::try_into)
            .transpose()?;

        Ok(Self {
            blueprint_dataset: value.blueprint_dataset.map(TryInto::try_into).transpose()?,
            asset_dataset: value.asset_dataset.map(TryInto::try_into).transpose()?,
            default_blueprint_segment,
            default_segment_table_blueprint_segment,
        })
    }
}

impl From<DatasetDetails> for crate::cloud::v1alpha1::DatasetDetails {
    fn from(value: DatasetDetails) -> Self {
        Self {
            blueprint_dataset: value.blueprint_dataset.map(Into::into),
            asset_dataset: value.asset_dataset.map(Into::into),
            default_blueprint_segment: value.default_blueprint_segment.clone().map(Into::into),
            default_segment_table_blueprint_segment: value
                .default_segment_table_blueprint_segment
                .clone()
                .map(Into::into),
        }
    }
}

// --- TableDetails ---

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableDetails {
    pub blueprint_dataset: Option<EntryId>,
    pub default_blueprint_segment: Option<SegmentId>,
}

impl TableDetails {
    /// Checks that the blueprint configuration is internally consistent.
    ///
    /// A default blueprint can only be set if a [`Self::blueprint_dataset`] is set too.
    ///
    /// This does *not* check that the blueprint dataset actually exists or is itself a blueprint
    /// dataset — callers with store access must verify that separately.
    pub fn validate_consistency(&self) -> Result<(), InconsistentBlueprintDetailsError> {
        if self.default_blueprint_segment.is_some() && self.blueprint_dataset.is_none() {
            return Err(InconsistentBlueprintDetailsError {
                entry_kind: "table",
            });
        }
        Ok(())
    }

    /// Returns the default blueprint for this table.
    ///
    /// Both `blueprint_dataset` and `default_blueprint_segment` must be set.
    pub fn default_blueprint(&self) -> Option<(EntryId, SegmentId)> {
        let blueprint = self.blueprint_dataset.as_ref()?;
        self.default_blueprint_segment
            .as_ref()
            .map(|default| (blueprint.clone(), default.clone()))
    }
}

impl TryFrom<crate::cloud::v1alpha1::TableDetails> for TableDetails {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::TableDetails) -> Result<Self, Self::Error> {
        Ok(Self {
            blueprint_dataset: value.blueprint_dataset.map(TryInto::try_into).transpose()?,
            default_blueprint_segment: value
                .default_blueprint_segment
                .map(TryInto::try_into)
                .transpose()?,
        })
    }
}

impl From<TableDetails> for crate::cloud::v1alpha1::TableDetails {
    fn from(value: TableDetails) -> Self {
        Self {
            blueprint_dataset: value.blueprint_dataset.map(Into::into),
            default_blueprint_segment: value.default_blueprint_segment.map(Into::into),
        }
    }
}

// --- DatasetEntry ---

#[derive(Debug, Clone)]
pub struct DatasetEntry {
    pub details: EntryDetails,
    pub dataset_details: DatasetDetails,
    pub handle: DatasetHandle,
}

impl TryFrom<crate::cloud::v1alpha1::DatasetEntry> for DatasetEntry {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::DatasetEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            details: value
                .details
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::DatasetEntry,
                    "details"
                ))?
                .try_into()?,
            dataset_details: value
                .dataset_details
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::DatasetDetails,
                    "dataset_details"
                ))?
                .try_into()?,
            handle: value
                .dataset_handle
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::DatasetEntry,
                    "handle"
                ))?
                .try_into()?,
        })
    }
}

impl From<DatasetEntry> for crate::cloud::v1alpha1::DatasetEntry {
    fn from(value: DatasetEntry) -> Self {
        Self {
            details: Some(value.details.into()),
            dataset_details: Some(value.dataset_details.into()),
            dataset_handle: Some(value.handle.into()),
        }
    }
}

// --- CreateDatasetEntryRequest ---

#[derive(Debug, Clone)]
pub struct CreateDatasetEntryRequest {
    /// Entry name (must be unique in catalog).
    pub name: EntryName,

    /// Override, use at your own risk.
    pub id: Option<EntryId>,
}

impl From<CreateDatasetEntryRequest> for crate::cloud::v1alpha1::CreateDatasetEntryRequest {
    fn from(value: CreateDatasetEntryRequest) -> Self {
        Self {
            name: Some(value.name.to_string()),
            id: value.id.map(Into::into),
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::CreateDatasetEntryRequest> for CreateDatasetEntryRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::CreateDatasetEntryRequest,
    ) -> Result<Self, Self::Error> {
        let name_str = value.name.ok_or(missing_field!(
            crate::cloud::v1alpha1::CreateDatasetEntryRequest,
            "name"
        ))?;
        Ok(Self {
            name: EntryName::new(name_str).map_err(TypeConversionError::InvalidEntryName)?,
            id: value.id.map(TryInto::try_into).transpose()?,
        })
    }
}

// --- CreateDatasetEntryResponse ---

#[derive(Debug, Clone)]
pub struct CreateDatasetEntryResponse {
    pub dataset: DatasetEntry,
}

impl From<CreateDatasetEntryResponse> for crate::cloud::v1alpha1::CreateDatasetEntryResponse {
    fn from(value: CreateDatasetEntryResponse) -> Self {
        Self {
            dataset: Some(value.dataset.into()),
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::CreateDatasetEntryResponse> for CreateDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::CreateDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset: value
                .dataset
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::CreateDatasetEntryResponse,
                    "dataset"
                ))?
                .try_into()?,
        })
    }
}

// --- CreateTableEntryRequest ---

#[derive(Debug, Clone)]
pub struct CreateTableEntryRequest {
    pub name: EntryName,
    pub schema: Schema,
    pub provider_details: Option<ProviderDetails>,
}

impl TryFrom<CreateTableEntryRequest> for crate::cloud::v1alpha1::CreateTableEntryRequest {
    type Error = TypeConversionError;
    fn try_from(value: CreateTableEntryRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name.to_string(),
            schema: Some((&value.schema).try_into()?),
            provider_details: value
                .provider_details
                .map(|d| (&d).try_into())
                .transpose()?,
        })
    }
}

impl TryFrom<&crate::cloud::v1alpha1::CreateTableEntryRequest> for CreateTableEntryRequest {
    type Error = TypeConversionError;
    fn try_from(
        value: &crate::cloud::v1alpha1::CreateTableEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            name: EntryName::new(value.name.clone())
                .map_err(TypeConversionError::InvalidEntryName)?,
            schema: value
                .schema
                .as_ref()
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::CreateTableEntryRequest,
                    "schema"
                ))?
                .try_into()?,
            provider_details: value
                .provider_details
                .as_ref()
                .map(|v| ProviderDetails::try_from(v))
                .transpose()?,
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::CreateTableEntryRequest> for CreateTableEntryRequest {
    type Error = TypeConversionError;
    fn try_from(
        value: crate::cloud::v1alpha1::CreateTableEntryRequest,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

// --- CreateTableEntryResponse ---

#[derive(Debug, Clone)]
pub struct CreateTableEntryResponse {
    pub table: TableEntry,
}

impl TryFrom<CreateTableEntryResponse> for crate::cloud::v1alpha1::CreateTableEntryResponse {
    type Error = TypeConversionError;
    fn try_from(value: CreateTableEntryResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            table: Some(value.table.try_into()?),
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::CreateTableEntryResponse> for CreateTableEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::CreateTableEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            table: value
                .table
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::CreateTableEntryResponse,
                    "table"
                ))?
                .try_into()?,
        })
    }
}

// --- ReadDatasetEntryResponse ---

#[derive(Debug, Clone)]
pub struct ReadDatasetEntryResponse {
    pub dataset_entry: DatasetEntry,
}

impl From<ReadDatasetEntryResponse> for crate::cloud::v1alpha1::ReadDatasetEntryResponse {
    fn from(value: ReadDatasetEntryResponse) -> Self {
        Self {
            dataset: Some(value.dataset_entry.into()),
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::ReadDatasetEntryResponse> for ReadDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::ReadDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_entry: value
                .dataset
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::ReadDatasetEntryResponse,
                    "dataset"
                ))?
                .try_into()?,
        })
    }
}

// --- UpdateDatasetEntryRequest ---

#[derive(Debug, Clone)]
pub struct UpdateDatasetEntryRequest {
    pub id: EntryId,
    pub dataset_details: DatasetDetails,
}

impl TryFrom<crate::cloud::v1alpha1::UpdateDatasetEntryRequest> for UpdateDatasetEntryRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::UpdateDatasetEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateDatasetEntryRequest,
                    "id"
                ))?
                .try_into()?,
            dataset_details: value
                .dataset_details
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateDatasetEntryRequest,
                    "dataset_details"
                ))?
                .try_into()?,
        })
    }
}

impl From<UpdateDatasetEntryRequest> for crate::cloud::v1alpha1::UpdateDatasetEntryRequest {
    fn from(value: UpdateDatasetEntryRequest) -> Self {
        Self {
            id: Some(value.id.into()),
            dataset_details: Some(value.dataset_details.into()),
        }
    }
}

// --- UpdateDatasetEntryResponse ---

#[derive(Debug, Clone)]
pub struct UpdateDatasetEntryResponse {
    pub dataset_entry: DatasetEntry,
}

impl From<UpdateDatasetEntryResponse> for crate::cloud::v1alpha1::UpdateDatasetEntryResponse {
    fn from(value: UpdateDatasetEntryResponse) -> Self {
        Self {
            dataset: Some(value.dataset_entry.into()),
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::UpdateDatasetEntryResponse> for UpdateDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::UpdateDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_entry: value
                .dataset
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateDatasetEntryResponse,
                    "dataset"
                ))?
                .try_into()?,
        })
    }
}

// --- DeleteEntryRequest ---

impl TryFrom<crate::cloud::v1alpha1::DeleteEntryRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::DeleteEntryRequest) -> Result<Self, Self::Error> {
        Ok(value
            .id
            .ok_or(missing_field!(
                crate::cloud::v1alpha1::DeleteEntryRequest,
                "id"
            ))?
            .try_into()?)
    }
}

// --- EntryDetailsUpdate ---

#[derive(Debug, Clone, Default)]
pub struct EntryDetailsUpdate {
    pub name: Option<EntryName>,
}

impl TryFrom<crate::cloud::v1alpha1::EntryDetailsUpdate> for EntryDetailsUpdate {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::EntryDetailsUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value
                .name
                .map(EntryName::new)
                .transpose()
                .map_err(TypeConversionError::InvalidEntryName)?,
        })
    }
}

impl From<EntryDetailsUpdate> for crate::cloud::v1alpha1::EntryDetailsUpdate {
    fn from(value: EntryDetailsUpdate) -> Self {
        Self {
            name: value.name.map(|name| name.to_string()),
        }
    }
}

// --- UpdateEntryRequest ---

#[derive(Debug, Clone)]
pub struct UpdateEntryRequest {
    pub id: re_log_types::EntryId,
    pub entry_details_update: EntryDetailsUpdate,
}

impl TryFrom<crate::cloud::v1alpha1::UpdateEntryRequest> for UpdateEntryRequest {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::UpdateEntryRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateEntryRequest,
                    "id"
                ))?
                .try_into()?,
            entry_details_update: value
                .entry_details_update
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateEntryRequest,
                    "entry_details_update"
                ))?
                .try_into()?,
        })
    }
}

impl From<UpdateEntryRequest> for crate::cloud::v1alpha1::UpdateEntryRequest {
    fn from(value: UpdateEntryRequest) -> Self {
        Self {
            id: Some(value.id.into()),
            entry_details_update: Some(value.entry_details_update.into()),
        }
    }
}

// --- UpdateEntryResponse ---

#[derive(Debug, Clone)]
pub struct UpdateEntryResponse {
    pub entry_details: EntryDetails,
}

impl TryFrom<crate::cloud::v1alpha1::UpdateEntryResponse> for UpdateEntryResponse {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::UpdateEntryResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            entry_details: value
                .entry_details
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateEntryResponse,
                    "entry_details"
                ))?
                .try_into()?,
        })
    }
}

impl From<UpdateEntryResponse> for crate::cloud::v1alpha1::UpdateEntryResponse {
    fn from(value: UpdateEntryResponse) -> Self {
        Self {
            entry_details: Some(value.entry_details.into()),
        }
    }
}

// --- ReadTableEntryRequest ---

impl TryFrom<crate::cloud::v1alpha1::ReadTableEntryRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::ReadTableEntryRequest) -> Result<Self, Self::Error> {
        Ok(value
            .id
            .ok_or(missing_field!(
                crate::cloud::v1alpha1::ReadTableEntryRequest,
                "id"
            ))?
            .try_into()?)
    }
}

// --- ReadTableEntryResponse ---

#[derive(Debug, Clone)]
pub struct ReadTableEntryResponse {
    pub table_entry: TableEntry,
}

impl TryFrom<ReadTableEntryResponse> for crate::cloud::v1alpha1::ReadTableEntryResponse {
    type Error = TypeConversionError;
    fn try_from(value: ReadTableEntryResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            table: Some(value.table_entry.try_into()?),
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::ReadTableEntryResponse> for ReadTableEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::ReadTableEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            table_entry: value
                .table
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::ReadTableEntryResponse,
                    "table_entry"
                ))?
                .try_into()?,
        })
    }
}

// --- UpdateTableEntryRequest ---

#[derive(Debug, Clone)]
pub struct UpdateTableEntryRequest {
    pub id: EntryId,
    pub table_details: TableDetails,
}

impl TryFrom<crate::cloud::v1alpha1::UpdateTableEntryRequest> for UpdateTableEntryRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::UpdateTableEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateTableEntryRequest,
                    "id"
                ))?
                .try_into()?,
            table_details: value
                .table_details
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateTableEntryRequest,
                    "table_details"
                ))?
                .try_into()?,
        })
    }
}

impl From<UpdateTableEntryRequest> for crate::cloud::v1alpha1::UpdateTableEntryRequest {
    fn from(value: UpdateTableEntryRequest) -> Self {
        Self {
            id: Some(value.id.into()),
            table_details: Some(value.table_details.into()),
        }
    }
}

// --- UpdateTableEntryResponse ---

#[derive(Debug, Clone)]
pub struct UpdateTableEntryResponse {
    pub table_entry: TableEntry,
}

impl TryFrom<UpdateTableEntryResponse> for crate::cloud::v1alpha1::UpdateTableEntryResponse {
    type Error = TypeConversionError;
    fn try_from(value: UpdateTableEntryResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            table: Some(value.table_entry.try_into()?),
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::UpdateTableEntryResponse> for UpdateTableEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::UpdateTableEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            table_entry: value
                .table
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::UpdateTableEntryResponse,
                    "table_entry"
                ))?
                .try_into()?,
        })
    }
}

// --- RegisterTableRequest ---

#[derive(Debug, Clone)]
pub struct RegisterTableRequest {
    pub name: EntryName,
    pub provider_details: ProviderDetails,
}

impl TryFrom<RegisterTableRequest> for crate::cloud::v1alpha1::RegisterTableRequest {
    type Error = TypeConversionError;
    fn try_from(value: RegisterTableRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name.to_string(),
            provider_details: Some((&value.provider_details).try_into()?),
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::RegisterTableRequest> for RegisterTableRequest {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::RegisterTableRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: EntryName::new(value.name).map_err(TypeConversionError::InvalidEntryName)?,
            provider_details: ProviderDetails::try_from(&value.provider_details.ok_or(
                missing_field!(
                    crate::cloud::v1alpha1::RegisterTableRequest,
                    "provider_details"
                ),
            )?)?,
        })
    }
}

// --- RegisterTableResponse ---

#[derive(Debug, Clone)]
pub struct RegisterTableResponse {
    pub table_entry: TableEntry,
}

impl TryFrom<crate::cloud::v1alpha1::RegisterTableResponse> for RegisterTableResponse {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::RegisterTableResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            table_entry: value
                .table_entry
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::RegisterTableResponse,
                    "table_entry"
                ))?
                .try_into()?,
        })
    }
}

// --- TableEntry ---

#[derive(Debug, Clone)]
pub struct TableEntry {
    pub details: EntryDetails,
    pub provider_details: ProviderDetails,
    pub table_details: TableDetails,
}

impl TryFrom<TableEntry> for crate::cloud::v1alpha1::TableEntry {
    type Error = TypeConversionError;
    fn try_from(value: TableEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            details: Some(value.details.into()),
            provider_details: Some((&value.provider_details).try_into()?),
            table_details: Some(value.table_details.into()),
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::TableEntry> for TableEntry {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::TableEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            details: value
                .details
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::TableEntry,
                    "details"
                ))?
                .try_into()?,
            provider_details: ProviderDetails::try_from(
                &value
                    .provider_details
                    .ok_or(missing_field!(crate::cloud::v1alpha1::TableEntry, "handle"))?,
            )?,
            table_details: value
                .table_details
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

// --- ProviderDetails ---

#[derive(Debug, Clone)]
pub enum ProviderDetails {
    SystemTable(SystemTable),
    LanceTable(LanceTable),
}

impl TryFrom<&prost_types::Any> for ProviderDetails {
    type Error = TypeConversionError;
    fn try_from(value: &prost_types::Any) -> Result<Self, Self::Error> {
        if value.type_url == crate::cloud::v1alpha1::LanceTable::type_url() {
            let as_proto = value.to_msg::<crate::cloud::v1alpha1::LanceTable>()?;
            let table = LanceTable::try_from(as_proto)?;
            Ok(Self::LanceTable(table))
        } else if value.type_url == crate::cloud::v1alpha1::SystemTable::type_url() {
            let as_proto = value.to_msg::<crate::cloud::v1alpha1::SystemTable>()?;
            let table = SystemTable::try_from(as_proto)?;
            Ok(Self::SystemTable(table))
        } else {
            Err(TypeConversionError::InvalidField {
                package_name: "rerun.cloud.v1alpha1",
                type_name: "ProviderDetails",
                field_name: "",
                reason: "enum value unspecified".to_owned(),
            })
        }
    }
}

impl TryFrom<&ProviderDetails> for prost_types::Any {
    type Error = TypeConversionError;
    fn try_from(value: &ProviderDetails) -> Result<Self, Self::Error> {
        match value {
            ProviderDetails::SystemTable(table) => {
                let as_proto: crate::cloud::v1alpha1::SystemTable = table.clone().into();
                Ok(prost_types::Any::from_msg(&as_proto)?)
            }
            ProviderDetails::LanceTable(table) => {
                let as_proto: crate::cloud::v1alpha1::LanceTable = table.clone().into();
                Ok(prost_types::Any::from_msg(&as_proto)?)
            }
        }
    }
}

impl ProviderDetails {
    pub fn type_url(&self) -> String {
        match self {
            Self::SystemTable(_) => crate::cloud::v1alpha1::SystemTable::type_url(),
            Self::LanceTable(_) => crate::cloud::v1alpha1::LanceTable::type_url(),
        }
    }
}

// --- SystemTable ---

#[derive(Debug, Clone)]
pub struct SystemTable {
    pub kind: crate::cloud::v1alpha1::SystemTableKind,
}

impl TryFrom<crate::cloud::v1alpha1::SystemTable> for SystemTable {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::SystemTable) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: value.kind.try_into()?,
        })
    }
}

impl From<SystemTable> for crate::cloud::v1alpha1::SystemTable {
    fn from(value: SystemTable) -> Self {
        Self {
            kind: value.kind as _,
        }
    }
}

// --- LanceTable ---

#[derive(Debug, Clone)]
pub struct LanceTable {
    pub table_url: url::Url,
}

impl TryFrom<crate::cloud::v1alpha1::LanceTable> for LanceTable {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::LanceTable) -> Result<Self, Self::Error> {
        Ok(Self {
            table_url: url::Url::parse(&value.table_url)?,
        })
    }
}

impl From<LanceTable> for crate::cloud::v1alpha1::LanceTable {
    fn from(value: LanceTable) -> Self {
        Self {
            table_url: value.table_url.to_string(),
        }
    }
}

// --- EntryKind ---

impl EntryKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            EntryKind::Dataset => "Dataset",
            EntryKind::Table => "Table",
            EntryKind::Unspecified => "Unspecified",
            EntryKind::DatasetView => "Dataset View",
            EntryKind::TableView => "Table View",
            EntryKind::BlueprintDataset => "Blueprint Dataset",
            EntryKind::AssetDataset => "Asset Dataset",
        }
    }
}

impl std::fmt::Display for EntryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// --- QueryDataset ---

#[derive(Debug, Default, Clone)]
pub struct Query {
    pub latest_at: Option<QueryLatestAt>,
    pub range: Option<QueryRange>,
    pub columns_always_include_everything: bool,
    pub columns_always_include_byte_offsets: bool,
    pub columns_always_include_entity_paths: bool,
    pub columns_always_include_static_indexes: bool,
    pub columns_always_include_global_indexes: bool,
    pub columns_always_include_component_indexes: bool,
}

impl Query {
    /// Create a query that returns everything that is needed to view every time point
    /// in the given range with latest-at semantics.
    pub fn latest_at_range(timeline_name: TimelineName, time_range: AbsoluteTimeRange) -> Self {
        Self {
            // So that we can show the state at the start:
            latest_at: Some(QueryLatestAt::global(Some(timeline_name), time_range.min)),
            // Show we can show everything in the range:
            range: Some(QueryRange {
                index: timeline_name,
                index_range: time_range.into(),
            }),
            ..Self::default()
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::Query> for Query {
    type Error = tonic::Status;

    fn try_from(value: crate::cloud::v1alpha1::Query) -> Result<Self, Self::Error> {
        let latest_at = value
            .latest_at
            .map(|latest_at| {
                Ok::<QueryLatestAt, tonic::Status>(QueryLatestAt {
                    index: latest_at
                        .index
                        .and_then(|index| index.timeline)
                        .map(|timeline| {
                            re_log_types::TimelineName::try_new(timeline.name)
                                .map_err(|err| tonic::Status::invalid_argument(err.to_string()))
                        })
                        .transpose()?,
                    at: latest_at
                        .at
                        .map(|at| TimeInt::new_temporal(at))
                        .unwrap_or_else(|| TimeInt::STATIC),
                    per_segment_values: latest_at
                        .per_segment_values
                        .into_iter()
                        .map(|ivl| ivl.values)
                        .collect(),
                })
            })
            .transpose()?;

        let range = value
            .range
            .map(|range| {
                Ok::<QueryRange, tonic::Status>(QueryRange {
                    index_range: range
                        .index_range
                        .ok_or_else(|| {
                            tonic::Status::invalid_argument(
                                "index_range is required for range query",
                            )
                        })?
                        .into(),
                    index: range
                        .index
                        .and_then(|index| index.timeline)
                        .ok_or_else(|| {
                            tonic::Status::invalid_argument("index is required for range query")
                        })
                        .and_then(|timeline| {
                            re_log_types::TimelineName::try_new(timeline.name)
                                .map_err(|err| tonic::Status::invalid_argument(err.to_string()))
                        })?,
                })
            })
            .transpose()?;

        Ok(Self {
            latest_at,
            range,
            columns_always_include_byte_offsets: value.columns_always_include_byte_offsets,
            columns_always_include_component_indexes: value
                .columns_always_include_component_indexes,
            columns_always_include_entity_paths: value.columns_always_include_entity_paths,
            columns_always_include_everything: value.columns_always_include_everything,
            columns_always_include_global_indexes: value.columns_always_include_global_indexes,
            columns_always_include_static_indexes: value.columns_always_include_static_indexes,
        })
    }
}

impl From<Query> for crate::cloud::v1alpha1::Query {
    fn from(value: Query) -> Self {
        crate::cloud::v1alpha1::Query {
            latest_at: value.latest_at.map(Into::into),
            range: value.range.map(|range| crate::cloud::v1alpha1::QueryRange {
                index: Some({
                    let timeline: TimelineName = range.index.into();
                    timeline.into()
                }),
                index_range: Some(range.index_range.into()),
            }),
            columns_always_include_byte_offsets: value.columns_always_include_byte_offsets,
            columns_always_include_component_indexes: value
                .columns_always_include_component_indexes,
            columns_always_include_entity_paths: value.columns_always_include_entity_paths,
            columns_always_include_everything: value.columns_always_include_everything,
            columns_always_include_global_indexes: value.columns_always_include_global_indexes,
            columns_always_include_static_indexes: value.columns_always_include_static_indexes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryLatestAt {
    /// Index name (timeline) to query.
    ///
    /// Use `None` for static only data.
    pub index: Option<TimelineName>,

    /// Global query value — applied to all segments.
    ///
    /// Use `TimeInt::STATIC` to query for static only data.
    /// Mutually exclusive with `per_segment_values`.
    pub at: TimeInt,

    /// Per-segment index values, positionally matched to
    /// `QueryDatasetRequest.segment_ids`.
    ///
    /// When non-empty, `at` and `Query.range` must be unset — the server
    /// reconstructs global bounds from these values.
    pub per_segment_values: Vec<Vec<i64>>,
}

impl QueryLatestAt {
    /// Construct a global `QueryLatestAt` (single value applied to all segments,
    /// no per-segment values).
    pub fn global(index: Option<TimelineName>, at: TimeInt) -> Self {
        Self {
            index,
            at,
            per_segment_values: Vec::new(),
        }
    }

    pub fn new_static() -> Self {
        Self::global(None, TimeInt::STATIC)
    }

    pub fn is_static(&self) -> bool {
        self.index.is_none()
    }
}

impl From<QueryLatestAt> for crate::cloud::v1alpha1::QueryLatestAt {
    fn from(value: QueryLatestAt) -> Self {
        // Map `TimeInt::STATIC` (sentinel for "unset") to `None` on the wire
        // so the receiving side's `at.map(TimeInt::new_temporal).unwrap_or(STATIC)`
        // round-trips correctly.
        let at = if value.at == TimeInt::STATIC {
            None
        } else {
            Some(value.at.as_i64())
        };
        crate::cloud::v1alpha1::QueryLatestAt {
            index: value.index.map(|index| {
                let timeline: TimelineName = index.into();
                timeline.into()
            }),
            at,
            per_segment_values: value
                .per_segment_values
                .into_iter()
                .map(|values| crate::cloud::v1alpha1::IndexValueList { values })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryRange {
    pub index: TimelineName,
    pub index_range: re_log_types::AbsoluteTimeRange,
}

// --- GetDatasetSchemaResponse ---

#[derive(Debug, thiserror::Error)]
pub enum GetDatasetSchemaResponseError {
    #[error(transparent)]
    ArrowError(#[from] ArrowError),

    #[error(transparent)]
    TypeConversionError(#[from] TypeConversionError),
}

impl GetDatasetSchemaResponse {
    pub fn schema(self) -> Result<Schema, GetDatasetSchemaResponseError> {
        Ok(self
            .schema
            .ok_or_else(|| {
                TypeConversionError::missing_field::<GetDatasetSchemaResponse>("schema")
            })?
            .try_into()?)
    }
}

// --- RegisterWithDatasetResponse ---

//TODO(ab): this should be an actual grpc message, returned by `RegisterWithDataset` instead of a dataframe
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RegisterWithDatasetTaskDescriptor {
    pub layer_name: LayerName,
    pub segment_id: SegmentId,
    pub segment_type: DataSourceKind,
    pub storage_url: url::Url,
    pub task_id: TaskId,
}

// --- UnregisterFromDatasetResponse ---

/// The dataframe follows the same schema as
/// [`ScanDatasetManifestDataframe`](crate::cloud::v1alpha1::ext::ScanDatasetManifestDataframe).
impl UnregisterFromDatasetResponse {
    pub fn data(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(Self, "data"))?)
    }
}

// --- ScanSegmentTableResponse --

// One row per segment; see [`ScanSegmentTableDataframe`].
impl ScanSegmentTableResponse {
    /// The inner field of the list columns, using arrow's conventional `"item"` name.
    pub fn list_item_field() -> FieldRef {
        lazy_field_ref!(Field::new("item", DataType::Utf8, false))
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        segment_ids: Vec<SegmentId>,
        layer_names: Vec<Vec<LayerName>>,
        storage_urls: Vec<Vec<String>>,
        last_updated_at: Vec<i64>,
        num_chunks: Vec<u64>,
        size_bytes: Vec<u64>,
    ) -> arrow::error::Result<RecordBatch> {
        ScanSegmentTableDataframe {
            rerun_segment_id: segment_ids.into(),
            rerun_layer_names: layer_names.into(),
            rerun_storage_urls: storage_urls.into(),
            rerun_last_updated_at: last_updated_at.into(),
            rerun_num_chunks: num_chunks.into(),
            rerun_size_bytes: size_bytes.into(),
            extra_columns: vec![],
        }
        .into_record_batch()
        .map_err(|err| ArrowError::InvalidArgumentError(err.to_string()))
    }

    pub fn data(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(Self, "data"))?)
    }
}

// --- ScanDatasetManifestResponse --

/// Column constants and helpers for the dataset manifest.
///
/// Terminology:
/// * A *layer* is a named slice of data that spans many segments (e.g. "base", "embeddings"),
///   with one source per segment it appears in.
/// * A *source* is a single `.rrd` (or, in the future, `.mcap` etc)
/// * A single segment is the concatenation of all the sources of all the layers it has data in.
///
/// The dataset manifest has one row per (layer, segment) pair,
/// i.e. a layer appears once per segment it has data in.
impl ScanDatasetManifestResponse {
    pub fn data(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(Self, "data"))?)
    }
}

// --- DataSource --

/// The file format of a [`DataSource`].
// NOTE: Match the values of the Protobuf definition to keep life simple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DataSourceKind {
    /// Rerun recording data (`.rrd` files).
    Rrd = 1,
}

impl std::fmt::Display for DataSourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rrd => write!(f, "rrd"),
        }
    }
}

impl From<DataSourceKind> for String {
    fn from(kind: DataSourceKind) -> Self {
        kind.to_string()
    }
}

impl TryFrom<crate::cloud::v1alpha1::DataSourceKind> for DataSourceKind {
    type Error = TypeConversionError;

    fn try_from(kind: crate::cloud::v1alpha1::DataSourceKind) -> Result<Self, Self::Error> {
        match kind {
            crate::cloud::v1alpha1::DataSourceKind::Rrd => Ok(Self::Rrd),

            crate::cloud::v1alpha1::DataSourceKind::Unspecified => {
                return Err(TypeConversionError::InvalidField {
                    package_name: "rerun.manifest_registry.v1alpha1",
                    type_name: "DataSourceKind",
                    field_name: "",
                    reason: "enum value unspecified".to_owned(),
                });
            }
        }
    }
}

impl TryFrom<i32> for DataSourceKind {
    type Error = TypeConversionError;

    fn try_from(kind: i32) -> Result<Self, Self::Error> {
        let kind = crate::cloud::v1alpha1::DataSourceKind::try_from(kind)?;
        kind.try_into()
    }
}

impl From<DataSourceKind> for crate::cloud::v1alpha1::DataSourceKind {
    fn from(value: DataSourceKind) -> Self {
        match value {
            DataSourceKind::Rrd => Self::Rrd,
        }
    }
}

impl DataSourceKind {
    pub fn to_arrow(self) -> ArrayRef {
        match self {
            Self::Rrd => {
                let rec_type = StringArray::from_iter_values(["rrd".to_owned()]);
                Arc::new(rec_type)
            }
        }
    }

    pub fn many_to_arrow(types: Vec<Self>) -> ArrayRef {
        let data = types
            .into_iter()
            .map(|typ| match typ {
                Self::Rrd => "rrd",
            })
            .collect::<Vec<_>>();
        Arc::new(StringArray::from(data))
    }

    pub fn from_arrow(array: &dyn Array) -> Result<Self, TypeConversionError> {
        let resource_type = array.try_downcast_array_ref::<StringArray>()?.value(0);

        match resource_type {
            "rrd" => Ok(Self::Rrd),
            _ => Err(TypeConversionError::ArrowError(
                ArrowError::InvalidArgumentError(format!("unknown resource type {resource_type}")),
            )),
        }
    }

    pub fn many_from_arrow(array: &dyn Array) -> Result<Vec<Self>, TypeConversionError> {
        let string_array = array.try_downcast_array_ref::<StringArray>()?;

        (0..string_array.len())
            .map(|i| {
                let resource_type = string_array.value(i);
                match resource_type {
                    "rrd" => Ok(Self::Rrd),
                    _ => Err(TypeConversionError::ArrowError(
                        ArrowError::InvalidArgumentError(format!(
                            "unknown resource type {resource_type}"
                        )),
                    )),
                }
            })
            .collect()
    }
}

#[test]
fn datasourcekind_roundtrip() {
    let kind = DataSourceKind::Rrd;
    let kind: crate::cloud::v1alpha1::DataSourceKind = kind.into();
    let kind = DataSourceKind::try_from(kind).unwrap();
    assert_eq!(DataSourceKind::Rrd, kind);
}

/// A pointer to one or more recording files stored in object storage.
///
/// A `DataSource` identifies a single file (when `is_prefix = false`) or
/// all files that share a common URL prefix (when `is_prefix = true`).
/// Every source belongs to a named [`LayerName`] within the dataset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataSource {
    /// URL of the recording file, or the common prefix when `is_prefix` is `true`.
    pub storage_url: url::Url,

    /// If `true`, `storage_url` is a prefix and matches all objects with that prefix.
    pub is_prefix: bool,

    /// The dataset layer this source belongs to (default: `"base"`).
    pub layer: LayerName,

    /// File format of the recording data.
    pub kind: DataSourceKind,
}

impl DataSource {
    pub const DEFAULT_LAYER: &str = LayerName::DEFAULT_STR;

    pub fn new_rrd(storage_url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            is_prefix: false,
            layer: LayerName::base(),
            kind: DataSourceKind::Rrd,
        })
    }

    pub fn new_rrd_prefix(storage_url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            is_prefix: true,
            layer: LayerName::base(),
            kind: DataSourceKind::Rrd,
        })
    }

    pub fn new_rrd_layer(
        layer: impl AsRef<str>,
        storage_url: impl AsRef<str>,
    ) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            is_prefix: false,
            layer: LayerName::new(layer.as_ref()),
            kind: DataSourceKind::Rrd,
        })
    }

    pub fn new_rrd_layer_prefix(
        layer: impl AsRef<str>,
        storage_url: impl AsRef<str>,
    ) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            is_prefix: true,
            layer: LayerName::new(layer.as_ref()),
            kind: DataSourceKind::Rrd,
        })
    }

    pub fn new_rrd_url(storage_url: url::Url) -> Self {
        Self {
            storage_url,
            is_prefix: false,
            layer: LayerName::base(),
            kind: DataSourceKind::Rrd,
        }
    }

    pub fn new_rrd_prefix_url(storage_url: url::Url) -> Self {
        Self {
            storage_url,
            is_prefix: true,
            layer: LayerName::base(),
            kind: DataSourceKind::Rrd,
        }
    }
}

impl From<DataSource> for crate::cloud::v1alpha1::DataSource {
    fn from(value: DataSource) -> Self {
        crate::cloud::v1alpha1::DataSource {
            storage_url: Some(value.storage_url.to_string()),
            prefix: value.is_prefix,
            layer: Some(value.layer.into()),
            typ: value.kind as i32,
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::DataSource> for DataSource {
    type Error = TypeConversionError;

    fn try_from(data_source: crate::cloud::v1alpha1::DataSource) -> Result<Self, Self::Error> {
        let storage_url = data_source
            .storage_url
            .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::DataSource, "storage_url"))?
            .parse()?;

        let layer = data_source
            .layer
            .map(LayerName::from)
            .unwrap_or_else(LayerName::base);

        let kind = DataSourceKind::try_from(data_source.typ)?;

        let prefix = data_source.prefix;

        Ok(Self {
            storage_url,
            is_prefix: prefix,
            layer,
            kind,
        })
    }
}

// --- Tasks ---

pub struct QueryTasksOnCompletionRequest {
    pub task_ids: Vec<TaskId>,
    pub timeout: std::time::Duration,
}

pub struct QueryTasksRequest {
    pub task_ids: Vec<TaskId>,
}

impl TryFrom<QueryTasksOnCompletionRequest>
    for crate::cloud::v1alpha1::QueryTasksOnCompletionRequest
{
    type Error = TypeConversionError;

    fn try_from(
        value: QueryTasksOnCompletionRequest,
    ) -> Result<crate::cloud::v1alpha1::QueryTasksOnCompletionRequest, Self::Error> {
        if value.task_ids.is_empty() {
            return Err(missing_field!(
                crate::cloud::v1alpha1::QueryTasksOnCompletionRequest,
                "task_ids"
            ));
        }
        let timeout: prost_types::Duration = value.timeout.try_into().map_err(|err| {
            invalid_field!(
                crate::cloud::v1alpha1::QueryTasksOnCompletionRequest,
                "timeout",
                err
            )
        })?;
        Ok(Self {
            ids: value.task_ids,
            timeout: Some(timeout),
        })
    }
}

impl TryFrom<QueryTasksRequest> for crate::cloud::v1alpha1::QueryTasksRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: QueryTasksRequest,
    ) -> Result<crate::cloud::v1alpha1::QueryTasksRequest, Self::Error> {
        Ok(Self {
            ids: value.task_ids,
        })
    }
}

// --

pub struct QueryTasksOnCompletionResponse {
    pub data: arrow::record_batch::RecordBatch,
}

impl TryFrom<crate::cloud::v1alpha1::QueryTasksOnCompletionResponse>
    for QueryTasksOnCompletionResponse
{
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::QueryTasksOnCompletionResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            data: value
                .data
                .ok_or_else(|| {
                    missing_field!(
                        crate::cloud::v1alpha1::QueryTasksOnCompletionResponse,
                        "data"
                    )
                })?
                .try_into()?,
        })
    }
}

// --

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TableInsertMode {
    Append,
    Overwrite,
    Replace,
    Update,
}

impl Default for TableInsertMode {
    fn default() -> Self {
        Self::Append
    }
}

impl TryFrom<i32> for TableInsertMode {
    type Error = TypeConversionError;

    fn try_from(value: i32) -> Result<Self, TypeConversionError> {
        let proto_value = crate::cloud::v1alpha1::TableInsertMode::try_from(value)?;
        Ok(Self::from(proto_value))
    }
}

impl From<crate::cloud::v1alpha1::TableInsertMode> for TableInsertMode {
    fn from(value: crate::cloud::v1alpha1::TableInsertMode) -> Self {
        use crate::cloud::v1alpha1 as cloud;
        match value {
            cloud::TableInsertMode::Unspecified | cloud::TableInsertMode::Append => Self::Append,
            cloud::TableInsertMode::Overwrite => Self::Overwrite,
            cloud::TableInsertMode::Replace => Self::Replace,
            cloud::TableInsertMode::Update => Self::Update,
        }
    }
}

impl From<TableInsertMode> for crate::cloud::v1alpha1::TableInsertMode {
    fn from(value: TableInsertMode) -> Self {
        match value {
            TableInsertMode::Append => Self::Append,
            TableInsertMode::Overwrite => Self::Overwrite,
            TableInsertMode::Replace => Self::Replace,
            TableInsertMode::Update => Self::Update,
        }
    }
}

// ---

/// Ergonomic counterpart to the codegen'd [`crate::cloud::v1alpha1::VersionResponse`].
pub struct VersionResponse {
    pub build_info: Option<re_build_info::BuildInfo>,
    pub version: String,
    pub cloud_provider: Option<String>,
    pub cloud_region: Option<String>,
    /// Server-supported feature flags. See `crate::cloud::v1alpha1::features`
    /// for known feature names. An empty list means the server is older than
    /// the client and feature-gated paths must fall back.
    pub features: Vec<String>,
}

impl VersionResponse {
    /// Whether the server advertises a given feature flag. Empty `features`
    /// (old server) returns false — callers must treat that as "fall back".
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }
}

impl From<crate::cloud::v1alpha1::VersionResponse> for VersionResponse {
    fn from(value: crate::cloud::v1alpha1::VersionResponse) -> Self {
        Self {
            build_info: value.build_info.map(Into::into),
            version: value.version,
            cloud_provider: value.cloud_provider,
            cloud_region: value.cloud_region,
            features: value.features,
        }
    }
}

#[cfg(test)]
mod tests {
    use arrow::datatypes::ToByteSlice as _;

    use super::*;
    use crate::cloud::v1alpha1::ext::{
        QueryTasksDataframe, RegisterWithDatasetDataframe, ScanDatasetManifestDataframe,
    };

    #[test]
    fn test_query_dataset_response_create_dataframe() {
        let chunk_ids = vec![re_chunk::ChunkId::new(), re_chunk::ChunkId::new()];
        let chunk_segment_ids = vec![
            SegmentId::from("segment_id_1"),
            SegmentId::from("segment_id_2"),
        ];
        let chunk_layer_names = vec![LayerName::from("layer1"), LayerName::from("layer2")];
        let chunk_keys = vec![b"key1".to_byte_slice(), b"key2".to_byte_slice()];
        let chunk_entity_paths = vec![EntityPath::root(), EntityPath::root()];
        let chunk_is_static = vec![true, false];
        let chunk_byte_lengths = vec![1024u64, 2048u64];
        let direct_urls = vec![None, None];
        let direct_urls_expiry = vec![None, None];

        let chunk_byte_lengths_uncompressed = vec![Some(2048u64), Some(4096u64)];

        let batch = QueryDatasetResponse::create_dataframe(
            chunk_ids,
            chunk_segment_ids,
            chunk_layer_names,
            chunk_keys,
            chunk_entity_paths,
            chunk_is_static,
            chunk_byte_lengths,
            chunk_byte_lengths_uncompressed,
            direct_urls,
            direct_urls_expiry,
        )
        .unwrap();

        assert_eq!(
            batch.schema().as_ref(),
            &QueryDatasetDataframe::max_schema(),
            "the builder must produce exactly the declared columns (no stray extra columns)"
        );
    }

    #[test]
    fn test_scan_segment_table_response_create_dataframe() {
        let segment_ids = vec![SegmentId::from("1"), SegmentId::from("2")];
        let layer_names = vec![
            vec![LayerName::from("a"), LayerName::from("b")],
            vec![LayerName::from("c")],
        ];
        let storage_urls = vec![vec!["d".to_owned(), "e".to_owned()], vec!["f".to_owned()]];
        let last_updated_at = vec![1, 2];
        let num_chunks = vec![1, 2];
        let size_bytes = vec![1, 2];

        let batch = ScanSegmentTableResponse::create_dataframe(
            segment_ids,
            layer_names,
            storage_urls,
            last_updated_at,
            num_chunks,
            size_bytes,
        )
        .unwrap();

        assert_eq!(
            batch.schema().as_ref(),
            &ScanSegmentTableDataframe::max_schema(),
            "the builder must produce exactly the declared columns (no stray extra columns)"
        );
    }

    #[test]
    fn test_query_latest_at_per_segment_values_round_trip() {
        let ext = QueryLatestAt {
            index: Some("frame".into()),
            at: TimeInt::STATIC,
            per_segment_values: vec![vec![1, 2, 3], vec![10], vec![]],
        };
        let wire: crate::cloud::v1alpha1::QueryLatestAt = ext.clone().into();
        assert!(wire.at.is_none(), "STATIC must serialize as None");
        assert_eq!(wire.per_segment_values.len(), 3);
        assert_eq!(wire.per_segment_values[0].values, vec![1, 2, 3]);
        assert_eq!(wire.per_segment_values[1].values, vec![10]);
        assert!(wire.per_segment_values[2].values.is_empty());
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_length_validates() {
        // Mismatched lengths → invalid_argument
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![
                crate::common::v1alpha1::SegmentId {
                    id: Some("00000000-0000-0000-0000-000000000001".to_owned()),
                },
                crate::common::v1alpha1::SegmentId {
                    id: Some("00000000-0000-0000-0000-000000000002".to_owned()),
                },
            ],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: Some(re_log_types::TimelineName::from("log_time").into()),
                    at: None,
                    per_segment_values: vec![crate::cloud::v1alpha1::IndexValueList {
                        values: vec![1],
                    }],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("per_segment_values.len()"));
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_requires_segment_ids() {
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: Some(re_log_types::TimelineName::from("log_time").into()),
                    at: None,
                    per_segment_values: vec![crate::cloud::v1alpha1::IndexValueList {
                        values: vec![1],
                    }],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("segment_ids"));
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_rejects_duplicate_segments() {
        let dup = crate::common::v1alpha1::SegmentId {
            id: Some("00000000-0000-0000-0000-000000000007".to_owned()),
        };
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![dup.clone(), dup],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: Some(re_log_types::TimelineName::from("log_time").into()),
                    at: None,
                    per_segment_values: vec![
                        crate::cloud::v1alpha1::IndexValueList { values: vec![1] },
                        crate::cloud::v1alpha1::IndexValueList { values: vec![2] },
                    ],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("duplicates"));
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_empty_is_allowed() {
        // per_segment_values absent → no validation triggered
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![],
            query: Some(crate::cloud::v1alpha1::Query::default()),
            ..Default::default()
        };
        QueryDatasetRequest::try_from(req).expect("empty per_segment_values must be accepted");
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_requires_index() {
        // Regression: previously the dataplatform handler errored on a missing
        // `latest_at.index` while OSS silently degraded to a static-only
        // fallback. Both backends must now reject identically at request decode.
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![crate::common::v1alpha1::SegmentId {
                id: Some("00000000-0000-0000-0000-000000000001".to_owned()),
            }],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: None,
                    at: None,
                    per_segment_values: vec![crate::cloud::v1alpha1::IndexValueList {
                        values: vec![1],
                    }],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("latest_at.index"));
    }

    #[test]
    fn test_version_response_has_feature() {
        let resp = VersionResponse {
            build_info: None,
            version: "1.2.3".to_owned(),
            cloud_provider: None,
            cloud_region: None,
            features: vec!["per_segment_index_values".to_owned(), "future_X".to_owned()],
        };
        assert!(resp.has_feature(crate::cloud::v1alpha1::features::PER_SEGMENT_INDEX_VALUES));
        assert!(resp.has_feature("future_X"));
        assert!(!resp.has_feature("nonexistent"));
    }

    #[test]
    fn test_version_response_old_server_empty_features() {
        // Old server returns features=vec![] (default for unknown field).
        let resp = VersionResponse {
            build_info: None,
            version: "0.5.0".to_owned(),
            cloud_provider: None,
            cloud_region: None,
            features: vec![],
        };
        assert!(!resp.has_feature(crate::cloud::v1alpha1::features::PER_SEGMENT_INDEX_VALUES));
    }

    #[test]
    fn test_features_constants_self_consistent() {
        // The advertise list must contain every constant we publish, and only those.
        let advertised = crate::cloud::v1alpha1::features::all_supported_features();
        assert!(
            advertised
                .contains(&crate::cloud::v1alpha1::features::PER_SEGMENT_INDEX_VALUES.to_owned())
        );
    }

    #[test]
    fn test_scan_dataset_manifest_dataframe() {
        let layer_name = vec![LayerName::from("a")];
        let segment_id = vec![SegmentId::from("1")];
        let storage_url = vec!["d".to_owned()];
        let layer_type = vec!["c".to_owned()];
        let registration_time = vec![1];
        let last_updated_at = vec![2];
        let num_chunks = vec![1];
        let size_bytes = vec![2];
        let schema_sha256 = vec![[1; 32]];
        let registration_status = vec![LayerRegistrationStatus::Done.to_string()];

        let batch = ScanDatasetManifestDataframe::new(
            layer_name,
            segment_id,
            storage_url,
            layer_type,
            registration_time,
            last_updated_at,
            num_chunks,
            size_bytes,
            schema_sha256,
            registration_status,
        )
        .into_record_batch()
        .unwrap();

        assert_eq!(
            batch.schema().as_ref(),
            &ScanDatasetManifestDataframe::max_schema(),
            "the builder must produce exactly the declared columns (no stray extra columns)"
        );
    }

    /// Snapshot-friendly schema description, in declared column order.
    fn format_schema(schema: &Schema) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for field in schema.fields() {
            let nullability = if field.is_nullable() {
                "nullable"
            } else {
                "non-null"
            };
            write!(
                &mut out,
                "{}: {nullability} {}",
                field.name(),
                field.data_type()
            )
            .expect("infallible");
            let metadata = field.metadata().iter().sorted().collect_vec();
            if metadata.is_empty() {
                out.push('\n');
            } else {
                out.push_str(" [\n");
                for (key, value) in metadata {
                    writeln!(&mut out, "    {key}: {value:?}").expect("infallible");
                }
                out.push_str("]\n");
            }
        }
        out
    }

    /// Pin the wire schemas of all dataframe responses, including column order,
    /// nullability, and field metadata.
    ///
    /// If one of these snapshots changes, you are changing the public wire format —
    /// make sure all consumers can handle it.
    #[test]
    fn dataframe_schema_snapshots() {
        insta::assert_snapshot!(
            "query_dataset_dataframe_schema",
            format_schema(&QueryDatasetDataframe::max_schema())
        );
        insta::assert_snapshot!(
            "query_tasks_dataframe_schema",
            format_schema(&QueryTasksDataframe::max_schema())
        );
        insta::assert_snapshot!(
            "register_with_dataset_dataframe_schema",
            format_schema(&RegisterWithDatasetDataframe::max_schema())
        );
        insta::assert_snapshot!(
            "scan_dataset_manifest_dataframe_schema",
            format_schema(&ScanDatasetManifestDataframe::max_schema())
        );
        insta::assert_snapshot!(
            "scan_segment_table_dataframe_schema",
            format_schema(&ScanSegmentTableDataframe::max_schema())
        );
    }
}
