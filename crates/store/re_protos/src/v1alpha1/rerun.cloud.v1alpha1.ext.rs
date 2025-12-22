use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BinaryArray, BooleanArray, FixedSizeBinaryBuilder, ListBuilder, RecordBatch,
    RecordBatchOptions, StringArray, StringBuilder, TimestampNanosecondArray, UInt8Array,
    UInt64Array,
};
use arrow::datatypes::{DataType, Field, FieldRef, Schema, TimeUnit};
use arrow::error::ArrowError;
use prost::Name as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::TimelineName;
use re_log_types::{AbsoluteTimeRange, external::re_types_core::ComponentBatch as _};
use re_log_types::{EntityPath, EntryId, TimeInt};
use re_sorbet::ComponentColumnDescriptor;

use crate::cloud::v1alpha1::{
    EntryKind, FetchChunksRequest, GetDatasetSchemaResponse, QueryDatasetResponse,
    QueryTasksResponse, RegisterWithDatasetResponse, ScanDatasetManifestResponse,
    ScanSegmentTableResponse, VectorDistanceMetric,
};
use crate::common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, SegmentId};
use crate::common::v1alpha1::{ComponentDescriptor, DataframePart, TaskId};
use crate::v1alpha1::rerun_common_v1alpha1_ext::ScanParameters;
use crate::{TypeConversionError, invalid_field, missing_field};

/// Helper to simplify writing `field_XXX() -> FieldRef` methods.
macro_rules! lazy_field_ref {
    ($fld:expr) => {{
        static FIELD: std::sync::OnceLock<FieldRef> = std::sync::OnceLock::new();
        let field = FIELD.get_or_init(|| Arc::new($fld));
        Arc::clone(field)
    }};
}

// --- CreateIndexRequest
#[derive(Debug)]
pub struct CreateIndexRequest {
    pub config: IndexConfig,
}

impl TryFrom<crate::cloud::v1alpha1::CreateIndexRequest> for CreateIndexRequest {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::CreateIndexRequest) -> Result<Self, Self::Error> {
        let crate::cloud::v1alpha1::CreateIndexRequest { config } = value;

        Ok(CreateIndexRequest {
            config: config
                .ok_or_else(|| {
                    missing_field!(crate::cloud::v1alpha1::CreateIndexRequest, "config")
                })?
                .try_into()?,
        })
    }
}

// --- RegisterWithDatasetRequest ---

#[derive(Debug)]
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
                .collect::<Result<Vec<_>, _>>()?,
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

// --- QueryDatasetRequest ---

#[derive(Debug, Clone)]
pub struct QueryDatasetRequest {
    pub segment_ids: Vec<crate::common::v1alpha1::ext::SegmentId>,
    pub chunk_ids: Vec<re_chunk::ChunkId>,
    pub entity_paths: Vec<EntityPath>,
    pub select_all_entity_paths: bool,
    pub fuzzy_descriptors: Vec<String>,
    pub exclude_static_data: bool,
    pub exclude_temporal_data: bool,
    pub scan_parameters: Option<crate::common::v1alpha1::ext::ScanParameters>,
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
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            segment_ids,

            chunk_ids: value
                .chunk_ids
                .into_iter()
                .map(|tuid| {
                    let id: re_tuid::Tuid = tuid.try_into()?;
                    Ok::<_, tonic::Status>(re_chunk::ChunkId::from_u128(id.as_u128()))
                })
                .collect::<Result<Vec<_>, _>>()?,

            entity_paths: value
                .entity_paths
                .into_iter()
                .map(|path| {
                    path.try_into().map_err(|err| {
                        tonic::Status::invalid_argument(format!("invalid entity path: {err}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,

            select_all_entity_paths: value.select_all_entity_paths,

            fuzzy_descriptors: value.fuzzy_descriptors,

            exclude_static_data: value.exclude_static_data,
            exclude_temporal_data: value.exclude_temporal_data,

            scan_parameters: value
                .scan_parameters
                .map(|params| params.try_into())
                .transpose()?,

            query: value.query.map(|q| q.try_into()).transpose()?,
        })
    }
}

// --- QueryDatasetResponse ---

impl QueryDatasetResponse {
    // These columns are guaranteed to be returned by `QueryDataset`. Additional columns may also be
    // returned.
    pub const FIELD_CHUNK_ID: &str = "chunk_id";
    pub const FIELD_CHUNK_SEGMENT_ID: &str = "chunk_segment_id";
    pub const FIELD_CHUNK_LAYER_NAME: &str = "rerun_segment_layer";
    pub const FIELD_CHUNK_KEY: &str = "chunk_key";
    pub const FIELD_CHUNK_ENTITY_PATH: &str = "chunk_entity_path";
    pub const FIELD_CHUNK_IS_STATIC: &str = "chunk_is_static";
    pub const FIELD_CHUNK_BYTE_LENGTH: &str = "chunk_byte_len";

    pub fn field_chunk_id() -> FieldRef {
        lazy_field_ref!(
            Field::new(Self::FIELD_CHUNK_ID, DataType::FixedSizeBinary(16), false).with_metadata(
                [(
                    re_sorbet::metadata::RERUN_KIND.to_owned(),
                    "control".to_owned()
                )]
                .into_iter()
                .collect(),
            )
        )
    }

    pub fn field_chunk_segment_id() -> FieldRef {
        lazy_field_ref!(
            Field::new(Self::FIELD_CHUNK_SEGMENT_ID, DataType::Utf8, false).with_metadata(
                [(
                    re_sorbet::metadata::RERUN_KIND.to_owned(),
                    "control".to_owned()
                )]
                .into_iter()
                .collect(),
            )
        )
    }

    pub fn field_chunk_layer_name() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_CHUNK_LAYER_NAME,
            DataType::Utf8,
            false
        ))
    }

    pub fn field_chunk_key() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_CHUNK_KEY, DataType::Binary, false))
    }

    pub fn field_chunk_entity_path() -> FieldRef {
        lazy_field_ref!(
            Field::new(Self::FIELD_CHUNK_ENTITY_PATH, DataType::Utf8, false).with_metadata(
                [(
                    re_sorbet::metadata::RERUN_KIND.to_owned(),
                    "control".to_owned()
                )]
                .into_iter()
                .collect(),
            )
        )
    }

    pub fn field_chunk_is_static() -> FieldRef {
        lazy_field_ref!(
            Field::new(Self::FIELD_CHUNK_IS_STATIC, DataType::Boolean, false).with_metadata(
                [(
                    re_sorbet::metadata::RERUN_KIND.to_owned(),
                    "control".to_owned()
                )]
                .into_iter()
                .collect(),
            )
        )
    }

    pub fn field_chunk_byte_len() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_CHUNK_BYTE_LENGTH,
            DataType::UInt64,
            false
        ))
    }

    pub fn fields() -> Vec<FieldRef> {
        vec![
            Self::field_chunk_id(),
            Self::field_chunk_segment_id(),
            Self::field_chunk_layer_name(),
            Self::field_chunk_key(),
            Self::field_chunk_entity_path(),
            Self::field_chunk_is_static(),
            Self::field_chunk_byte_len(),
        ]
    }

    pub fn schema() -> arrow::datatypes::Schema {
        Schema::new(Self::fields())
    }

    pub fn create_empty_dataframe() -> RecordBatch {
        let schema = Arc::new(Self::schema());
        RecordBatch::new_empty(schema)
    }

    pub fn create_dataframe(
        chunk_ids: Vec<re_chunk::ChunkId>,
        chunk_segment_ids: Vec<String>,
        chunk_layer_names: Vec<String>,
        chunk_keys: Vec<&[u8]>,
        chunk_entity_paths: Vec<String>,
        chunk_is_static: Vec<bool>,
        chunk_byte_lengths: Vec<u64>,
    ) -> arrow::error::Result<RecordBatch> {
        let schema = Arc::new(Self::schema());

        let columns: Vec<ArrayRef> = vec![
            chunk_ids
                .to_arrow()
                .expect("to_arrow for ChunkIds never fails"),
            Arc::new(StringArray::from(chunk_segment_ids)),
            Arc::new(StringArray::from(chunk_layer_names)),
            Arc::new(BinaryArray::from(chunk_keys)),
            Arc::new(StringArray::from(chunk_entity_paths)),
            Arc::new(BooleanArray::from(chunk_is_static)),
            Arc::new(UInt64Array::from(chunk_byte_lengths)),
        ];

        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(chunk_ids.len())),
        )
    }
}

impl FetchChunksRequest {
    // This is the only required column in the request.
    pub const FIELD_CHUNK_KEY: &str = QueryDatasetResponse::FIELD_CHUNK_KEY;

    //TODO(RR-2677): actually, these are also required for now.
    pub const FIELD_CHUNK_ID: &str = QueryDatasetResponse::FIELD_CHUNK_ID;
    pub const FIELD_CHUNK_SEGMENT_ID: &str = QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID;
    pub const FIELD_CHUNK_LAYER_NAME: &str = QueryDatasetResponse::FIELD_CHUNK_LAYER_NAME;
    pub const FIELD_CHUNK_BYTE_LENGTH: &str = QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH;

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

    pub fn field_chunk_id() -> FieldRef {
        QueryDatasetResponse::field_chunk_id()
    }

    pub fn field_chunk_segment_id() -> FieldRef {
        QueryDatasetResponse::field_chunk_segment_id()
    }

    pub fn field_chunk_layer_name() -> FieldRef {
        QueryDatasetResponse::field_chunk_layer_name()
    }

    pub fn field_chunk_key() -> FieldRef {
        QueryDatasetResponse::field_chunk_key()
    }

    pub fn fields() -> Vec<FieldRef> {
        vec![
            Self::field_chunk_id(),
            Self::field_chunk_segment_id(),
            Self::field_chunk_layer_name(),
            Self::field_chunk_key(),
        ]
    }

    pub fn schema() -> arrow::datatypes::Schema {
        Schema::new(Self::fields())
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

// --- Tasks ---

impl QueryTasksResponse {
    pub const FIELD_TASK_ID: &str = "task_id";
    pub const FIELD_KIND: &str = "kind";
    pub const FIELD_DATA: &str = "data";
    pub const FIELD_EXEC_STATUS: &str = "exec_status";
    pub const FIELD_MSGS: &str = "msgs";
    pub const FIELD_BLOB_LEN: &str = "blob_len";
    pub const FIELD_LEASE_OWNER: &str = "lease_owner";
    pub const FIELD_LEASE_EXPIRATION: &str = "lease_expiration";
    pub const FIELD_ATTEMPTS: &str = "attempts";
    pub const FIELD_CREATION_TIME: &str = "creation_time";
    pub const FIELD_LAST_UPDATE_TIME: &str = "last_update_time";

    pub fn dataframe_part(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(QueryTasksResponse, "data"))?)
    }

    pub fn schema() -> arrow::datatypes::Schema {
        Schema::new(vec![
            Field::new(Self::FIELD_TASK_ID, DataType::Utf8, false),
            Field::new(Self::FIELD_KIND, DataType::Utf8, true),
            Field::new(Self::FIELD_DATA, DataType::Utf8, true),
            Field::new(Self::FIELD_EXEC_STATUS, DataType::Utf8, false),
            Field::new(Self::FIELD_MSGS, DataType::Utf8, true),
            Field::new(Self::FIELD_BLOB_LEN, DataType::UInt64, true),
            Field::new(Self::FIELD_LEASE_OWNER, DataType::Utf8, true),
            Field::new(
                Self::FIELD_LEASE_EXPIRATION,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(Self::FIELD_ATTEMPTS, DataType::UInt8, false),
            Field::new(
                Self::FIELD_CREATION_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(
                Self::FIELD_LAST_UPDATE_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
        ])
    }

    pub fn create_dataframe(
        task_ids: Vec<String>,
        kind: Vec<Option<String>>,
        data: Vec<Option<String>>,
        exec_status: Vec<String>,
        msgs: Vec<Option<String>>,
        blob_len: Vec<Option<u64>>,
        lease_owner: Vec<Option<String>>,
        lease_expiration: Vec<Option<i64>>,
        attempts: Vec<u8>,
        creation_time: Vec<Option<i64>>,
        last_update_time: Vec<Option<i64>>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = task_ids.len();
        let schema = Arc::new(Self::schema());

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(task_ids)),
            Arc::new(StringArray::from(kind)),
            Arc::new(StringArray::from(data)),
            Arc::new(StringArray::from(exec_status)),
            Arc::new(StringArray::from(msgs)),
            Arc::new(UInt64Array::from(blob_len)),
            Arc::new(StringArray::from(lease_owner)),
            Arc::new(TimestampNanosecondArray::from(lease_expiration)),
            Arc::new(UInt8Array::from(attempts)),
            Arc::new(TimestampNanosecondArray::from(creation_time)),
            Arc::new(TimestampNanosecondArray::from(last_update_time)),
        ];

        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
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

// --- EntryDetails ---

#[derive(Debug, Clone)]
pub struct EntryDetails {
    pub id: re_log_types::EntryId,
    pub name: String,
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
            name: value
                .name
                .ok_or(missing_field!(crate::cloud::v1alpha1::EntryDetails, "name"))?,
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
            name: Some(value.name),
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

// --- DatasetDetails ---

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DatasetDetails {
    pub blueprint_dataset: Option<EntryId>,
    pub default_blueprint_segment: Option<SegmentId>,
}

impl DatasetDetails {
    /// Returns the default blueprint for this dataset.
    ///
    /// Both `blueprint_dataset` and `default_blueprint_segment` must be set.
    pub fn default_blueprint(&self) -> Option<(EntryId, SegmentId)> {
        let blueprint = self.blueprint_dataset.as_ref()?;
        self.default_blueprint_segment
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

        Ok(Self {
            blueprint_dataset: value.blueprint_dataset.map(TryInto::try_into).transpose()?,
            default_blueprint_segment,
        })
    }
}

impl From<DatasetDetails> for crate::cloud::v1alpha1::DatasetDetails {
    fn from(value: DatasetDetails) -> Self {
        Self {
            blueprint_dataset: value.blueprint_dataset.map(Into::into),
            default_blueprint_segment: value.default_blueprint_segment.clone().map(Into::into),
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
    pub name: String,

    /// Override, use at your own risk.
    pub id: Option<EntryId>,
}

impl From<CreateDatasetEntryRequest> for crate::cloud::v1alpha1::CreateDatasetEntryRequest {
    fn from(value: CreateDatasetEntryRequest) -> Self {
        Self {
            name: Some(value.name),
            id: value.id.map(Into::into),
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::CreateDatasetEntryRequest> for CreateDatasetEntryRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::CreateDatasetEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name.ok_or(missing_field!(
                crate::cloud::v1alpha1::CreateDatasetEntryRequest,
                "name"
            ))?,

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
    pub name: String,
    pub schema: Schema,
    pub provider_details: Option<ProviderDetails>,
}

impl TryFrom<CreateTableEntryRequest> for crate::cloud::v1alpha1::CreateTableEntryRequest {
    type Error = TypeConversionError;
    fn try_from(value: CreateTableEntryRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            schema: Some((&value.schema).try_into()?),
            provider_details: value
                .provider_details
                .map(|d| (&d).try_into())
                .transpose()?,
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::CreateTableEntryRequest> for CreateTableEntryRequest {
    type Error = TypeConversionError;
    fn try_from(
        value: crate::cloud::v1alpha1::CreateTableEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            schema: value
                .schema
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::CreateTableEntryRequest,
                    "schema"
                ))?
                .try_into()?,
            provider_details: value
                .provider_details
                .map(|v| ProviderDetails::try_from(&v))
                .transpose()?,
        })
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
    pub name: Option<String>,
}

impl TryFrom<crate::cloud::v1alpha1::EntryDetailsUpdate> for EntryDetailsUpdate {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::EntryDetailsUpdate) -> Result<Self, Self::Error> {
        Ok(Self { name: value.name })
    }
}

impl From<EntryDetailsUpdate> for crate::cloud::v1alpha1::EntryDetailsUpdate {
    fn from(value: EntryDetailsUpdate) -> Self {
        Self { name: value.name }
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

// --- RegisterTableRequest ---

#[derive(Debug, Clone)]
pub struct RegisterTableRequest {
    pub name: String,
    pub provider_details: ProviderDetails,
}

impl TryFrom<RegisterTableRequest> for crate::cloud::v1alpha1::RegisterTableRequest {
    type Error = TypeConversionError;
    fn try_from(value: RegisterTableRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            provider_details: Some((&value.provider_details).try_into()?),
        })
    }
}

impl TryFrom<crate::cloud::v1alpha1::RegisterTableRequest> for RegisterTableRequest {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::RegisterTableRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
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
}

impl TryFrom<TableEntry> for crate::cloud::v1alpha1::TableEntry {
    type Error = TypeConversionError;
    fn try_from(value: TableEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            details: Some(value.details.into()),
            provider_details: Some((&value.provider_details).try_into()?),
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
    pub fn latest_at_range(timeline_name: &TimelineName, time_range: AbsoluteTimeRange) -> Self {
        Self {
            // So that we can show the state at the start:
            latest_at: Some(QueryLatestAt {
                index: Some(timeline_name.to_string()),
                at: time_range.min,
            }),
            // Show we can show everything in the range:
            range: Some(QueryRange {
                index: timeline_name.to_string(),
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
                        .and_then(|index| index.timeline.map(|timeline| timeline.name)),
                    at: latest_at
                        .at
                        .map(|at| TimeInt::new_temporal(at))
                        .unwrap_or_else(|| TimeInt::STATIC),
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
                        .and_then(|index| index.timeline.map(|timeline| timeline.name))
                        .ok_or_else(|| {
                            tonic::Status::invalid_argument("index is required for range query")
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
    pub index: Option<String>,

    /// The timestamp to query at.
    ///
    /// Use `TimeInt::STATIC` to query for static only data.
    pub at: TimeInt,
}

impl QueryLatestAt {
    pub fn new_static() -> Self {
        Self {
            index: None,
            at: TimeInt::STATIC,
        }
    }

    pub fn is_static(&self) -> bool {
        self.index.is_none()
    }
}

impl From<QueryLatestAt> for crate::cloud::v1alpha1::QueryLatestAt {
    fn from(value: QueryLatestAt) -> Self {
        crate::cloud::v1alpha1::QueryLatestAt {
            index: value.index.map(|index| {
                let timeline: TimelineName = index.into();
                timeline.into()
            }),
            at: Some(value.at.as_i64()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryRange {
    pub index: String,
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

impl RegisterWithDatasetResponse {
    pub const FIELD_SEGMENT_ID: &str = "rerun_segment_id";
    pub const FIELD_SEGMENT_LAYER: &str = "rerun_segment_layer";
    pub const FIELD_SEGMENT_TYPE: &str = "rerun_segment_type";
    pub const FIELD_STORAGE_URL: &str = "rerun_storage_url";
    pub const FIELD_TASK_ID: &str = "rerun_task_id";

    /// The Arrow schema of the dataframe in [`Self::data`].
    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::FIELD_SEGMENT_ID, DataType::Utf8, false),
            Field::new(Self::FIELD_SEGMENT_LAYER, DataType::Utf8, false),
            Field::new(Self::FIELD_SEGMENT_TYPE, DataType::Utf8, false),
            Field::new(Self::FIELD_STORAGE_URL, DataType::Utf8, false),
            Field::new(Self::FIELD_TASK_ID, DataType::Utf8, false),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        segment_ids: Vec<String>,
        segment_layers: Vec<String>,
        segment_types: Vec<String>,
        storage_urls: Vec<String>,
        task_ids: Vec<String>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = segment_ids.len();
        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(segment_ids)),
            Arc::new(StringArray::from(segment_layers)),
            Arc::new(StringArray::from(segment_types)),
            Arc::new(StringArray::from(storage_urls)),
            Arc::new(StringArray::from(task_ids)),
        ];

        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }
}

//TODO(ab): this should be an actual grpc message, returned by `RegisterWithDataset` instead of a dataframe
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RegisterWithDatasetTaskDescriptor {
    pub segment_id: SegmentId,
    pub segment_type: DataSourceKind,
    pub storage_url: url::Url,
    pub task_id: TaskId,
}

// --- ScanSegmentTableResponse --

impl ScanSegmentTableResponse {
    pub const FIELD_SEGMENT_ID: &str = "rerun_segment_id";

    /// Layer names for this segment, one per layer.
    ///
    /// Should have the same length as [`Self::FIELD_STORAGE_URLS`].
    pub const FIELD_LAYER_NAMES: &str = "rerun_layer_names";

    /// Storage URLs for this segment, one per layer.
    ///
    /// Should have the same length as [`Self::FIELD_LAYER_NAMES`].
    pub const FIELD_STORAGE_URLS: &str = "rerun_storage_urls";

    /// Keeps track of the most recent time any layer belonging to this segment was updated in any
    /// way.
    pub const FIELD_LAST_UPDATED_AT: &str = "rerun_last_updated_at";

    /// Total number of chunks for this segment.
    pub const FIELD_NUM_CHUNKS: &str = "rerun_num_chunks";

    /// Total size in bytes for this segment.
    pub const FIELD_SIZE_BYTES: &str = "rerun_size_bytes";

    pub fn field_segment_id() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_SEGMENT_ID, DataType::Utf8, false))
    }

    pub fn field_layer_names() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_LAYER_NAMES,
            DataType::List(Self::field_layer_names_inner()),
            false,
        ))
    }

    pub fn field_layer_names_inner() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_LAYER_NAMES, DataType::Utf8, false))
    }

    pub fn field_storage_urls() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_STORAGE_URLS,
            DataType::List(Self::field_storage_urls_inner()),
            false,
        ))
    }

    pub fn field_storage_urls_inner() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_STORAGE_URLS, DataType::Utf8, false))
    }

    pub fn field_last_updated_at() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_LAST_UPDATED_AT,
            DataType::Timestamp(TimeUnit::Nanosecond, None),
            false,
        ))
    }

    pub fn field_num_chunks() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_NUM_CHUNKS, DataType::UInt64, false))
    }

    pub fn field_size_bytes() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_SIZE_BYTES, DataType::UInt64, false))
    }

    // NOTE: changing this method is a breaking change for implementation (aka it at least breaks
    // tests in `dataplatform`)
    pub fn fields() -> Vec<FieldRef> {
        vec![
            Self::field_segment_id(),
            Self::field_layer_names(),
            Self::field_storage_urls(),
            Self::field_last_updated_at(),
            Self::field_num_chunks(),
            Self::field_size_bytes(),
        ]
    }

    pub fn schema() -> Schema {
        Schema::new(Self::fields())
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        segment_ids: Vec<String>,
        layer_names: Vec<Vec<String>>,
        storage_urls: Vec<Vec<String>>,
        last_updated_at: Vec<i64>,
        num_chunks: Vec<u64>,
        size_bytes: Vec<u64>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = segment_ids.len();
        let schema = Arc::new(Self::schema());

        let mut layer_names_builder =
            ListBuilder::new(StringBuilder::new()).with_field(Self::field_layer_names_inner());

        for mut inner_vec in layer_names {
            for layer_name in inner_vec.drain(..) {
                layer_names_builder.values().append_value(layer_name)
            }
            layer_names_builder.append(true);
        }

        let mut urls_builder =
            ListBuilder::new(StringBuilder::new()).with_field(Self::field_storage_urls_inner());

        for mut inner_vec in storage_urls {
            for layer_name in inner_vec.drain(..) {
                urls_builder.values().append_value(layer_name)
            }
            urls_builder.append(true);
        }

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(segment_ids)),
            Arc::new(layer_names_builder.finish()),
            Arc::new(urls_builder.finish()),
            Arc::new(TimestampNanosecondArray::from(last_updated_at)),
            Arc::new(UInt64Array::from(num_chunks)),
            Arc::new(UInt64Array::from(size_bytes)),
        ];

        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }

    pub fn data(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(Self, "data"))?)
    }
}

// --- ScanDatasetManifestResponse --

impl ScanDatasetManifestResponse {
    pub const FIELD_LAYER_NAME: &str = "rerun_layer_name";
    pub const FIELD_SEGMENT_ID: &str = "rerun_segment_id";
    pub const FIELD_STORAGE_URL: &str = "rerun_storage_url";
    pub const FIELD_LAYER_TYPE: &str = "rerun_layer_type";

    /// Time at which the layer was initially registered.
    pub const FIELD_REGISTRATION_TIME: &str = "rerun_registration_time";

    /// When was this row of the manifest modified last?
    pub const FIELD_LAST_UPDATED_AT: &str = "rerun_last_updated_at";
    pub const FIELD_NUM_CHUNKS: &str = "rerun_num_chunks";
    pub const FIELD_SIZE_BYTES: &str = "rerun_size_bytes";
    pub const FIELD_SCHEMA_SHA256: &str = "rerun_schema_sha256";

    pub fn field_layer_name() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_LAYER_NAME, DataType::Utf8, false))
    }

    pub fn field_segment_id() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_SEGMENT_ID, DataType::Utf8, false))
    }

    pub fn field_storage_url() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_STORAGE_URL, DataType::Utf8, false))
    }

    pub fn field_layer_type() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_LAYER_TYPE, DataType::Utf8, false))
    }

    pub fn field_registration_time() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_REGISTRATION_TIME,
            DataType::Timestamp(TimeUnit::Nanosecond, None),
            false
        ))
    }

    pub fn field_last_updated_at() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_LAST_UPDATED_AT,
            DataType::Timestamp(TimeUnit::Nanosecond, None),
            false
        ))
    }

    pub fn field_num_chunks() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_NUM_CHUNKS, DataType::UInt64, false))
    }

    pub fn field_size_bytes() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_SIZE_BYTES, DataType::UInt64, false))
    }

    pub fn field_schema_sha256() -> FieldRef {
        lazy_field_ref!(Field::new(
            Self::FIELD_SCHEMA_SHA256,
            DataType::FixedSizeBinary(32),
            false
        ))
    }

    pub fn fields() -> Vec<FieldRef> {
        vec![
            Self::field_layer_name(),
            Self::field_segment_id(),
            Self::field_storage_url(),
            Self::field_layer_type(),
            Self::field_registration_time(),
            Self::field_last_updated_at(),
            Self::field_num_chunks(),
            Self::field_size_bytes(),
            Self::field_schema_sha256(),
        ]
    }

    pub fn schema() -> Schema {
        Schema::new(Self::fields())
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        layer_names: Vec<String>,
        segment_ids: Vec<String>,
        storage_urls: Vec<String>,
        layer_types: Vec<String>,
        registration_times: Vec<i64>,
        last_updated_at_times: Vec<i64>,
        num_chunks: Vec<u64>,
        size_bytes: Vec<u64>,
        schema_sha256s: Vec<[u8; 32]>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = segment_ids.len();
        let schema = Arc::new(Self::schema());

        let mut schema_sha256_builder = FixedSizeBinaryBuilder::with_capacity(row_count, 32);
        for sha256 in schema_sha256s {
            schema_sha256_builder.append_value(sha256.as_slice())?;
        }

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(layer_names)),
            Arc::new(StringArray::from(segment_ids)),
            Arc::new(StringArray::from(storage_urls)),
            Arc::new(StringArray::from(layer_types)),
            Arc::new(TimestampNanosecondArray::from(registration_times)),
            Arc::new(TimestampNanosecondArray::from(last_updated_at_times)),
            Arc::new(UInt64Array::from(num_chunks)),
            Arc::new(UInt64Array::from(size_bytes)),
            Arc::new(schema_sha256_builder.finish()),
        ];

        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }

    pub fn data(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(Self, "data"))?)
    }
}

// --- DataSource --

// NOTE: Match the values of the Protobuf definition to keep life simple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DataSourceKind {
    Rrd = 1,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataSource {
    pub storage_url: url::Url,
    pub is_prefix: bool,
    pub layer: String,
    pub kind: DataSourceKind,
}

impl DataSource {
    pub const DEFAULT_LAYER: &str = "base";

    pub fn new_rrd(storage_url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            is_prefix: false,
            layer: Self::DEFAULT_LAYER.to_owned(),
            kind: DataSourceKind::Rrd,
        })
    }

    pub fn new_rrd_prefix(storage_url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
            is_prefix: true,
            layer: Self::DEFAULT_LAYER.to_owned(),
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
            layer: layer.as_ref().into(),
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
            layer: layer.as_ref().into(),
            kind: DataSourceKind::Rrd,
        })
    }
}

impl From<DataSource> for crate::cloud::v1alpha1::DataSource {
    fn from(value: DataSource) -> Self {
        crate::cloud::v1alpha1::DataSource {
            storage_url: Some(value.storage_url.to_string()),
            prefix: value.is_prefix,
            layer: Some(value.layer),
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
            .unwrap_or_else(|| Self::DEFAULT_LAYER.to_owned());

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

// ---

impl std::fmt::Display for VectorDistanceMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Unspecified => "unspecified",
            Self::L2 => "l2",
            Self::Cosine => "cosine",
            Self::Dot => "dot",
            Self::Hamming => "hamming",
        };

        f.write_str(s)
    }
}

impl std::str::FromStr for VectorDistanceMetric {
    type Err = TypeConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "l2" => Self::L2,
            "cosine" => Self::Cosine,
            "Dot" => Self::Dot,
            "Hamming" => Self::Hamming,
            _ => {
                return Err(invalid_field!(
                    crate::cloud::v1alpha1::IndexProperties,
                    "VectorDistanceMetric",
                    &format!("{s:?} is not a valid value"),
                ));
            }
        })
    }
}

// ---

/// Depending on the type of index that is being created, different properties
/// can be specified. These are defined by `IndexProperties`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IndexProperties {
    Inverted {
        store_position: bool,
        base_tokenizer: String,
    },

    VectorIvfPq {
        // see proto file for documentation
        target_partition_num_rows: Option<u32>,
        num_sub_vectors: u32,
        metric: VectorDistanceMetric,
    },

    Btree,
}

impl std::fmt::Display for IndexProperties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inverted {
                store_position,
                base_tokenizer,
            } => write!(
                f,
                "Inverted {{ store_position: {store_position}, base_tokenizer: {base_tokenizer} }}"
            ),

            Self::VectorIvfPq {
                target_partition_num_rows,
                num_sub_vectors,
                metric,
            } => {
                if let Some(target_partition_num_rows) = target_partition_num_rows {
                    write!(
                        f,
                        "VectorIvfPq {{ target_partition_num_rows: {target_partition_num_rows}, num_sub_vectors: {num_sub_vectors}, metric: {metric} }}"
                    )
                } else {
                    write!(
                        f,
                        "VectorIvfPq {{ num_sub_vectors: {num_sub_vectors}, metric: {metric} }}"
                    )
                }
            }

            Self::Btree => write!(f, "Btree"),
        }
    }
}

/// Convert `IndexProperties` into its equivalent storage model
impl From<IndexProperties> for crate::cloud::v1alpha1::IndexProperties {
    fn from(other: IndexProperties) -> Self {
        match other {
            IndexProperties::Btree => Self {
                props: Some(crate::cloud::v1alpha1::index_properties::Props::Btree(
                    crate::cloud::v1alpha1::BTreeIndex {},
                )),
            },
            IndexProperties::Inverted {
                store_position,
                base_tokenizer,
            } => Self {
                props: Some(crate::cloud::v1alpha1::index_properties::Props::Inverted(
                    crate::cloud::v1alpha1::InvertedIndex {
                        store_position: Some(store_position),
                        base_tokenizer: Some(base_tokenizer),
                    },
                )),
            },
            IndexProperties::VectorIvfPq {
                target_partition_num_rows,
                num_sub_vectors,
                metric,
            } => Self {
                props: Some(crate::cloud::v1alpha1::index_properties::Props::Vector(
                    crate::cloud::v1alpha1::VectorIvfPqIndex {
                        target_partition_num_rows,
                        num_sub_vectors: Some(num_sub_vectors),
                        distance_metrics: metric.into(),
                    },
                )),
            },
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::IndexProperties> for IndexProperties {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::IndexProperties) -> Result<Self, Self::Error> {
        let props = value
            .props
            .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::IndexProperties, "props"))?;

        use crate::cloud::v1alpha1::index_properties::Props;

        match props {
            Props::Inverted(data) => Ok(Self::Inverted {
                store_position: data.store_position.ok_or_else(|| {
                    missing_field!(
                        crate::cloud::v1alpha1::IndexProperties,
                        "props.store_position"
                    )
                })?,
                base_tokenizer: data.base_tokenizer.ok_or_else(|| {
                    missing_field!(
                        crate::cloud::v1alpha1::IndexProperties,
                        "props.base_tokenizer"
                    )
                })?,
            }),

            Props::Vector(data) => Ok(Self::VectorIvfPq {
                target_partition_num_rows: data.target_partition_num_rows,
                num_sub_vectors: data.num_sub_vectors.ok_or_else(|| {
                    missing_field!(
                        crate::cloud::v1alpha1::IndexProperties,
                        "props.num_sub_vectors"
                    )
                })?,

                metric: data.distance_metrics(),
            }),

            Props::Btree(_) => Ok(Self::Btree),
        }
    }
}

// ---

/// Depending on the type of index that is being queried, different properties
/// can be specified.
#[derive(Debug, Clone)]
pub enum IndexQueryProperties {
    Inverted,
    Vector { top_k: u32 },
    Btree,
}

impl TryFrom<crate::cloud::v1alpha1::IndexQueryProperties> for IndexQueryProperties {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::IndexQueryProperties) -> Result<Self, Self::Error> {
        let props = value
            .props
            .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::IndexQueryProperties, "props"))?;

        use crate::cloud::v1alpha1::index_query_properties::Props;

        match props {
            Props::Inverted(_) => Ok(Self::Inverted),

            Props::Vector(vector_index_query) => Ok(Self::Vector {
                top_k: vector_index_query.top_k.ok_or_else(|| {
                    missing_field!(crate::cloud::v1alpha1::VectorIndexQuery, "top_k")
                })?,
            }),

            Props::Btree(_) => Ok(Self::Btree),
        }
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndexColumn {
    pub entity_path: re_chunk::EntityPath,
    pub descriptor: re_types_core::ComponentDescriptor,
}

impl std::fmt::Display for IndexColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.entity_path, self.descriptor.display_name())
    }
}

impl TryFrom<crate::cloud::v1alpha1::IndexColumn> for IndexColumn {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::IndexColumn) -> Result<Self, Self::Error> {
        Ok(Self {
            entity_path: value
                .entity_path
                .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::IndexColumn, "entity_path"))?
                .try_into()?,
            descriptor: value
                .component
                .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::IndexColumn, "component"))?
                .try_into()?,
        })
    }
}

impl From<ComponentColumnDescriptor> for IndexColumn {
    fn from(value: ComponentColumnDescriptor) -> Self {
        let descriptor = value.component_descriptor();
        IndexColumn {
            entity_path: value.entity_path,
            descriptor,
        }
    }
}

impl From<IndexColumn> for crate::cloud::v1alpha1::IndexColumn {
    fn from(value: IndexColumn) -> Self {
        Self {
            entity_path: Some(value.entity_path.into()),
            component: Some(value.descriptor.into()),
        }
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexConfig {
    pub time_index: re_log_types::TimelineName,
    pub column: IndexColumn,
    pub properties: IndexProperties,
}

impl TryFrom<crate::cloud::v1alpha1::IndexConfig> for IndexConfig {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::IndexConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            time_index: value
                .time_index
                .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::IndexConfig, "time_index"))?
                .try_into()?,
            column: value
                .column
                .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::IndexConfig, "column"))?
                .try_into()?,
            properties: value
                .properties
                .ok_or_else(|| missing_field!(crate::cloud::v1alpha1::IndexConfig, "properties"))?
                .try_into()?,
        })
    }
}

impl From<IndexConfig> for crate::cloud::v1alpha1::IndexConfig {
    fn from(value: IndexConfig) -> Self {
        Self {
            properties: Some(value.properties.into()),
            column: Some(value.column.into()),
            time_index: Some(value.time_index.into()),
        }
    }
}

// ---

#[derive(Debug, Clone)]
pub struct SearchDatasetRequest {
    pub column: IndexColumn,
    pub query: RecordBatch,
    pub properties: IndexQueryProperties,
    pub scan_parameters: ScanParameters,
}

impl TryFrom<crate::cloud::v1alpha1::SearchDatasetRequest> for SearchDatasetRequest {
    type Error = TypeConversionError;
    fn try_from(value: crate::cloud::v1alpha1::SearchDatasetRequest) -> Result<Self, Self::Error> {
        Ok(SearchDatasetRequest {
            column: value
                .column
                .ok_or_else(|| {
                    missing_field!(crate::cloud::v1alpha1::SearchDatasetRequest, "column")
                })?
                .try_into()?,
            query: value
                .query
                .ok_or_else(|| {
                    missing_field!(crate::cloud::v1alpha1::SearchDatasetRequest, "query")
                })?
                .try_into()?,
            properties: value
                .properties
                .ok_or_else(|| {
                    missing_field!(crate::cloud::v1alpha1::SearchDatasetRequest, "properties")
                })?
                .try_into()?,
            scan_parameters: value
                .scan_parameters
                .map(ScanParameters::try_from)
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

// ---

impl From<ComponentColumnDescriptor> for crate::cloud::v1alpha1::IndexColumn {
    fn from(value: ComponentColumnDescriptor) -> Self {
        Self {
            entity_path: Some(value.entity_path.into()),

            component: Some(ComponentDescriptor {
                archetype: value.archetype.map(|n| n.full_name().to_owned()),
                component: Some(value.component.to_string()),
                component_type: value.component_type.map(|c| c.full_name().to_owned()),
            }),
        }
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
        }
    }
}

impl From<TableInsertMode> for crate::cloud::v1alpha1::TableInsertMode {
    fn from(value: TableInsertMode) -> Self {
        match value {
            TableInsertMode::Append => Self::Append,
            TableInsertMode::Overwrite => Self::Overwrite,
            TableInsertMode::Replace => Self::Replace,
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use arrow::datatypes::ToByteSlice as _;

    use super::*;

    #[test]
    fn test_query_dataset_response_create_dataframe() {
        let chunk_ids = vec![re_chunk::ChunkId::new(), re_chunk::ChunkId::new()];
        let chunk_segment_ids = vec!["segment_id_1".to_owned(), "segment_id_2".to_owned()];
        let chunk_layer_names = vec!["layer1".to_owned(), "layer2".to_owned()];
        let chunk_keys = vec![b"key1".to_byte_slice(), b"key2".to_byte_slice()];
        let chunk_entity_paths = vec!["/".to_owned(), "/".to_owned()];
        let chunk_is_static = vec![true, false];
        let chunk_byte_lengths = vec![1024u64, 2048u64];

        QueryDatasetResponse::create_dataframe(
            chunk_ids,
            chunk_segment_ids,
            chunk_layer_names,
            chunk_keys,
            chunk_entity_paths,
            chunk_is_static,
            chunk_byte_lengths,
        )
        .unwrap();
    }

    /// Ensure `crate_dataframe` implementation is consistent with `schema()`
    #[test]
    fn test_scan_segment_table_response_create_dataframe() {
        let segment_ids = vec!["1".to_owned(), "2".to_owned()];
        let layer_names = vec![vec!["a".to_owned(), "b".to_owned()], vec!["c".to_owned()]];
        let storage_urls = vec![vec!["d".to_owned(), "e".to_owned()], vec!["f".to_owned()]];
        let last_updated_at = vec![1, 2];
        let num_chunks = vec![1, 2];
        let size_bytes = vec![1, 2];

        ScanSegmentTableResponse::create_dataframe(
            segment_ids,
            layer_names,
            storage_urls,
            last_updated_at,
            num_chunks,
            size_bytes,
        )
        .unwrap();
    }

    /// Ensure `crate_dataframe` implementation is consistent with `schema()`
    #[test]
    fn test_scan_dataset_manifest_response_create_dataframe() {
        let layer_name = vec!["a".to_owned()];
        let segment_id = vec!["1".to_owned()];
        let storage_url = vec!["d".to_owned()];
        let layer_type = vec!["c".to_owned()];
        let registration_time = vec![1];
        let last_updated_at = vec![2];
        let num_chunks = vec![1];
        let size_bytes = vec![2];
        let schema_sha256 = vec![[1; 32]];

        ScanDatasetManifestResponse::create_dataframe(
            layer_name,
            segment_id,
            storage_url,
            layer_type,
            registration_time,
            last_updated_at,
            num_chunks,
            size_bytes,
            schema_sha256,
        )
        .unwrap();
    }
}
