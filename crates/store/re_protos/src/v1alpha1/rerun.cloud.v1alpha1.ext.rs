use std::sync::Arc;

use arrow::array::{
    BinaryArray, BooleanArray, FixedSizeBinaryBuilder, ListBuilder, RecordBatchOptions,
    StringBuilder, UInt8Array, UInt64Array,
};
use arrow::datatypes::FieldRef;
use arrow::{
    array::{Array, ArrayRef, RecordBatch, StringArray, TimestampNanosecondArray},
    datatypes::{DataType, Field, Schema, TimeUnit},
    error::ArrowError,
};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::TimelineName;
use re_log_types::external::re_types_core::ComponentBatch as _;
use re_log_types::{EntityPath, EntryId, TimeInt};
use re_sorbet::ComponentColumnDescriptor;

use crate::cloud::v1alpha1::{
    EntryKind, FetchChunksRequest, GetDatasetSchemaResponse, QueryDatasetResponse,
    QueryTasksResponse, RegisterWithDatasetResponse, ScanDatasetManifestResponse,
    ScanPartitionTableResponse, VectorDistanceMetric,
};
use crate::common::v1alpha1::{
    ComponentDescriptor, DataframePart, TaskId,
    ext::{DatasetHandle, IfDuplicateBehavior, PartitionId},
};
use crate::{TypeConversionError, invalid_field, missing_field};

/// Helper to simplify writing `field_XXX() -> FieldRef` methods.
macro_rules! lazy_field_ref {
    ($fld:expr) => {{
        static FIELD: std::sync::OnceLock<FieldRef> = std::sync::OnceLock::new();
        let field = FIELD.get_or_init(|| Arc::new($fld));
        Arc::clone(field)
    }};
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
    pub partition_ids: Vec<crate::common::v1alpha1::ext::PartitionId>,
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
            partition_ids: vec![],
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
            partition_ids: value.partition_ids.into_iter().map(Into::into).collect(),
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
        Ok(Self {
            partition_ids: value
                .partition_ids
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,

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
    pub const FIELD_CHUNK_PARTITION_ID: &str = "chunk_partition_id";
    pub const FIELD_CHUNK_LAYER_NAME: &str = "rerun_partition_layer";
    pub const FIELD_CHUNK_KEY: &str = "chunk_key";
    pub const FIELD_CHUNK_ENTITY_PATH: &str = "chunk_entity_path";
    pub const FIELD_CHUNK_IS_STATIC: &str = "chunk_is_static";

    pub fn field_chunk_id() -> FieldRef {
        lazy_field_ref!(
            Field::new(Self::FIELD_CHUNK_ID, DataType::FixedSizeBinary(16), false).with_metadata(
                [("rerun:kind".to_owned(), "control".to_owned())]
                    .into_iter()
                    .collect(),
            )
        )
    }

    pub fn field_chunk_partition_id() -> FieldRef {
        lazy_field_ref!(
            Field::new(Self::FIELD_CHUNK_PARTITION_ID, DataType::Utf8, false).with_metadata(
                [("rerun:kind".to_owned(), "control".to_owned())]
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
                [("rerun:kind".to_owned(), "control".to_owned())]
                    .into_iter()
                    .collect(),
            )
        )
    }

    pub fn field_chunk_is_static() -> FieldRef {
        lazy_field_ref!(
            Field::new(Self::FIELD_CHUNK_IS_STATIC, DataType::Boolean, false).with_metadata(
                [("rerun:kind".to_owned(), "control".to_owned())]
                    .into_iter()
                    .collect(),
            )
        )
    }

    pub fn fields() -> Vec<FieldRef> {
        vec![
            Self::field_chunk_id(),
            Self::field_chunk_partition_id(),
            Self::field_chunk_layer_name(),
            Self::field_chunk_key(),
            Self::field_chunk_entity_path(),
            Self::field_chunk_is_static(),
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
        chunk_partition_ids: Vec<String>,
        chunk_layer_names: Vec<String>,
        chunk_keys: Vec<&[u8]>,
        chunk_entity_paths: Vec<String>,
        chunk_is_static: Vec<bool>,
    ) -> arrow::error::Result<RecordBatch> {
        let schema = Arc::new(Self::schema());

        let columns: Vec<ArrayRef> = vec![
            chunk_ids
                .to_arrow()
                .expect("to_arrow for ChunkIds never fails"),
            Arc::new(StringArray::from(chunk_partition_ids)),
            Arc::new(StringArray::from(chunk_layer_names)),
            Arc::new(BinaryArray::from(chunk_keys)),
            Arc::new(StringArray::from(chunk_entity_paths)),
            Arc::new(BooleanArray::from(chunk_is_static)),
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
    pub const FIELD_CHUNK_PARTITION_ID: &str = QueryDatasetResponse::FIELD_CHUNK_PARTITION_ID;
    pub const FIELD_CHUNK_LAYER_NAME: &str = QueryDatasetResponse::FIELD_CHUNK_LAYER_NAME;

    pub fn required_column_names() -> Vec<String> {
        vec![
            Self::FIELD_CHUNK_KEY.to_owned(),
            //TODO(RR-2677): remove these
            Self::FIELD_CHUNK_ID.to_owned(),
            Self::FIELD_CHUNK_PARTITION_ID.to_owned(),
            Self::FIELD_CHUNK_LAYER_NAME.to_owned(),
        ]
    }

    pub fn field_chunk_id() -> FieldRef {
        QueryDatasetResponse::field_chunk_id()
    }

    pub fn field_chunk_partition_id() -> FieldRef {
        QueryDatasetResponse::field_chunk_partition_id()
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
            Self::field_chunk_partition_id(),
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

#[derive(Debug, Clone, Default)]
pub struct DatasetDetails {
    pub blueprint_dataset: Option<EntryId>,
    pub default_blueprint: Option<PartitionId>,
}

impl DatasetDetails {
    /// Returns the default blueprint for this dataset.
    ///
    /// Both `blueprint_dataset` and `default_blueprint` must be set.
    pub fn default_bluprint(&self) -> Option<(EntryId, PartitionId)> {
        let blueprint = self.blueprint_dataset.as_ref()?;
        self.default_blueprint
            .as_ref()
            .map(|default| (blueprint.clone(), default.clone()))
    }
}

impl TryFrom<crate::cloud::v1alpha1::DatasetDetails> for DatasetDetails {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::DatasetDetails) -> Result<Self, Self::Error> {
        Ok(Self {
            blueprint_dataset: value.blueprint_dataset.map(TryInto::try_into).transpose()?,
            default_blueprint: value.default_blueprint.map(TryInto::try_into).transpose()?,
        })
    }
}

impl From<DatasetDetails> for crate::cloud::v1alpha1::DatasetDetails {
    fn from(value: DatasetDetails) -> Self {
        Self {
            blueprint_dataset: value.blueprint_dataset.map(Into::into),
            default_blueprint: value.default_blueprint.map(Into::into),
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

impl TryFrom<crate::cloud::v1alpha1::CreateDatasetEntryRequest> for String {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::CreateDatasetEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(value.name.ok_or(missing_field!(
            crate::cloud::v1alpha1::CreateDatasetEntryRequest,
            "name"
        ))?)
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

impl From<ReadTableEntryResponse> for crate::cloud::v1alpha1::ReadTableEntryResponse {
    fn from(value: ReadTableEntryResponse) -> Self {
        Self {
            table: Some(value.table_entry.into()),
        }
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
    pub provider_details: prost_types::Any,
}

impl From<RegisterTableRequest> for crate::cloud::v1alpha1::RegisterTableRequest {
    fn from(value: RegisterTableRequest) -> Self {
        Self {
            name: value.name,
            provider_details: Some(value.provider_details),
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::RegisterTableRequest> for RegisterTableRequest {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::RegisterTableRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            provider_details: value.provider_details.ok_or(missing_field!(
                crate::cloud::v1alpha1::RegisterTableRequest,
                "provider_details"
            ))?,
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
    pub provider_details: prost_types::Any,
}

impl From<TableEntry> for crate::cloud::v1alpha1::TableEntry {
    fn from(value: TableEntry) -> Self {
        Self {
            details: Some(value.details.into()),
            provider_details: Some(value.provider_details),
        }
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
            provider_details: value
                .provider_details
                .ok_or(missing_field!(crate::cloud::v1alpha1::TableEntry, "handle"))?,
        })
    }
}

// --- ProviderDetails ---

pub trait ProviderDetails {
    fn try_as_any(&self) -> Result<prost_types::Any, TypeConversionError>;

    fn try_from_any(any: &prost_types::Any) -> Result<Self, TypeConversionError>
    where
        Self: Sized;
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

impl ProviderDetails for SystemTable {
    fn try_as_any(&self) -> Result<prost_types::Any, TypeConversionError> {
        let as_proto: crate::cloud::v1alpha1::SystemTable = self.clone().into();
        Ok(prost_types::Any::from_msg(&as_proto)?)
    }

    fn try_from_any(any: &prost_types::Any) -> Result<Self, TypeConversionError> {
        let as_proto = any.to_msg::<crate::cloud::v1alpha1::SystemTable>()?;
        Ok(as_proto.try_into()?)
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

impl ProviderDetails for LanceTable {
    fn try_as_any(&self) -> Result<prost_types::Any, TypeConversionError> {
        let as_proto: crate::cloud::v1alpha1::LanceTable = self.clone().into();
        Ok(prost_types::Any::from_msg(&as_proto)?)
    }

    fn try_from_any(any: &prost_types::Any) -> Result<Self, TypeConversionError> {
        let as_proto = any.to_msg::<crate::cloud::v1alpha1::LanceTable>()?;
        Ok(as_proto.try_into()?)
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
    pub columns_always_include_chunk_ids: bool,
    pub columns_always_include_byte_offsets: bool,
    pub columns_always_include_entity_paths: bool,
    pub columns_always_include_static_indexes: bool,
    pub columns_always_include_global_indexes: bool,
    pub columns_always_include_component_indexes: bool,
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
            columns_always_include_chunk_ids: value.columns_always_include_chunk_ids,
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
            columns_always_include_chunk_ids: value.columns_always_include_chunk_ids,
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
    pub const PARTITION_ID: &str = "rerun_partition_id";
    pub const PARTITION_LAYER: &str = "rerun_partition_layer";
    pub const PARTITION_TYPE: &str = "rerun_partition_type";
    pub const STORAGE_URL: &str = "rerun_storage_url";
    pub const TASK_ID: &str = "rerun_task_id";

    /// The Arrow schema of the dataframe in [`Self::data`].
    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::PARTITION_ID, DataType::Utf8, false),
            Field::new(Self::PARTITION_LAYER, DataType::Utf8, false),
            Field::new(Self::PARTITION_TYPE, DataType::Utf8, false),
            Field::new(Self::STORAGE_URL, DataType::Utf8, false),
            Field::new(Self::TASK_ID, DataType::Utf8, false),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        partition_layers: Vec<String>,
        partition_types: Vec<String>,
        storage_urls: Vec<String>,
        task_ids: Vec<String>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = partition_ids.len();
        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(partition_ids)),
            Arc::new(StringArray::from(partition_layers)),
            Arc::new(StringArray::from(partition_types)),
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
#[derive(Debug)]
pub struct RegisterWithDatasetTaskDescriptor {
    pub partition_id: PartitionId,
    pub partition_type: DataSourceKind,
    pub storage_url: url::Url,
    pub task_id: TaskId,
}

// --- ScanPartitionTableResponse --

impl ScanPartitionTableResponse {
    pub const FIELD_PARTITION_ID: &str = "rerun_partition_id";

    /// Layer names for this partition, one per layer.
    ///
    /// Should have the same length as [`Self::FIELD_STORAGE_URLS`].
    pub const FIELD_LAYER_NAMES: &str = "rerun_layer_names";

    /// Storage URLs for this partition, one per layer.
    ///
    /// Should have the same length as [`Self::FIELD_LAYER_NAMES`].
    pub const FIELD_STORAGE_URLS: &str = "rerun_storage_urls";

    /// Keeps track of the most recent time any layer belonging to this partition was updated in any
    /// way.
    pub const FIELD_LAST_UPDATED_AT: &str = "rerun_last_updated_at";

    /// Total number of chunks for this partition.
    pub const FIELD_NUM_CHUNKS: &str = "rerun_num_chunks";

    /// Total size in bytes for this partition.
    pub const FIELD_SIZE_BYTES: &str = "rerun_size_bytes";

    pub fn field_layer_names_inner() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_LAYER_NAMES, DataType::Utf8, false))
    }

    pub fn field_storage_urls_inner() -> FieldRef {
        lazy_field_ref!(Field::new(Self::FIELD_STORAGE_URLS, DataType::Utf8, false))
    }

    // NOTE: changing this method is a breaking change for implementation (aka it at least breaks
    // tests in `dataplatform`)
    pub fn fields() -> Vec<Field> {
        vec![
            Field::new(Self::FIELD_PARTITION_ID, DataType::Utf8, false),
            Field::new(
                Self::FIELD_LAYER_NAMES,
                DataType::List(Self::field_layer_names_inner()),
                false,
            ),
            Field::new(
                Self::FIELD_STORAGE_URLS,
                DataType::List(Self::field_storage_urls_inner()),
                false,
            ),
            Field::new(
                Self::FIELD_LAST_UPDATED_AT,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(Self::FIELD_NUM_CHUNKS, DataType::UInt64, false),
            Field::new(Self::FIELD_SIZE_BYTES, DataType::UInt64, false),
        ]
    }

    pub fn schema() -> Schema {
        Schema::new(Self::fields())
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        layer_names: Vec<Vec<String>>,
        storage_urls: Vec<Vec<String>>,
        last_updated_at: Vec<i64>,
        num_chunks: Vec<u64>,
        size_bytes: Vec<u64>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = partition_ids.len();
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
            Arc::new(StringArray::from(partition_ids)),
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
    pub const FIELD_PARTITION_ID: &str = "rerun_partition_id";
    pub const FIELD_STORAGE_URL: &str = "rerun_storage_url";
    pub const FIELD_LAYER_TYPE: &str = "rerun_layer_type";

    /// Time at which the layer was initially registered.
    pub const FIELD_REGISTRATION_TIME: &str = "rerun_registration_time";

    /// When was this row of the manifest modified last?
    pub const FIELD_LAST_UPDATED_AT: &str = "rerun_last_updated_at";
    pub const FIELD_NUM_CHUNKS: &str = "rerun_num_chunks";
    pub const FIELD_SIZE_BYTES: &str = "rerun_size_bytes";
    pub const FIELD_SCHEMA_SHA256: &str = "rerun_schema_sha256";

    // NOTE: changing this method is a breaking change for implementation (aka it at least breaks
    // tests in `dataplatform`)
    pub fn fields() -> Vec<Field> {
        vec![
            Field::new(Self::FIELD_LAYER_NAME, DataType::Utf8, false),
            Field::new(Self::FIELD_PARTITION_ID, DataType::Utf8, false),
            Field::new(Self::FIELD_STORAGE_URL, DataType::Utf8, false),
            Field::new(Self::FIELD_LAYER_TYPE, DataType::Utf8, false),
            Field::new(
                Self::FIELD_REGISTRATION_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                Self::FIELD_LAST_UPDATED_AT,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(Self::FIELD_NUM_CHUNKS, DataType::UInt64, false),
            Field::new(Self::FIELD_SIZE_BYTES, DataType::UInt64, false),
            Field::new(
                Self::FIELD_SCHEMA_SHA256,
                DataType::FixedSizeBinary(32),
                false,
            ),
        ]
    }

    pub fn schema() -> Schema {
        Schema::new(Self::fields())
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        layer_names: Vec<String>,
        partition_ids: Vec<String>,
        storage_urls: Vec<String>,
        layer_types: Vec<String>,
        registration_times: Vec<i64>,
        last_updated_at_times: Vec<i64>,
        num_chunks: Vec<u64>,
        size_bytes: Vec<u64>,
        schema_sha256s: Vec<[u8; 32]>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = partition_ids.len();
        let schema = Arc::new(Self::schema());

        let mut schema_sha256_builder = FixedSizeBinaryBuilder::with_capacity(row_count, 32);
        for sha256 in schema_sha256s {
            schema_sha256_builder.append_value(sha256.as_slice())?;
        }

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(layer_names)),
            Arc::new(StringArray::from(partition_ids)),
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
    pub layer: String,
    pub kind: DataSourceKind,
}

impl DataSource {
    pub const DEFAULT_LAYER: &str = "base";

    pub fn new_rrd(storage_url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            storage_url: storage_url.as_ref().parse()?,
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
            layer: layer.as_ref().into(),
            kind: DataSourceKind::Rrd,
        })
    }
}

impl From<DataSource> for crate::cloud::v1alpha1::DataSource {
    fn from(value: DataSource) -> Self {
        crate::cloud::v1alpha1::DataSource {
            storage_url: Some(value.storage_url.to_string()),
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

        Ok(Self {
            storage_url,
            layer,
            kind,
        })
    }
}

/// Depending on the type of index that is being created, different properties
/// can be specified. These are defined by `IndexProperties`.
#[derive(Debug, Clone)]
pub enum IndexProperties {
    Inverted {
        store_position: bool,
        base_tokenizer: String,
    },
    VectorIvfPq {
        num_partitions: usize,
        num_sub_vectors: usize,
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
                num_partitions,
                num_sub_vectors,
                metric,
            } => write!(
                f,
                "VectorIvfPq {{ num_partitions: {num_partitions}, num_sub_vectors: {num_sub_vectors}, metric: {metric:?} }}"
            ),
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
                num_partitions,
                num_sub_vectors,
                metric,
            } => Self {
                props: Some(crate::cloud::v1alpha1::index_properties::Props::Vector(
                    crate::cloud::v1alpha1::VectorIvfPqIndex {
                        num_partitions: Some(num_partitions as u32),
                        num_sub_vectors: Some(num_sub_vectors as u32),
                        distance_metrics: metric.into(),
                    },
                )),
            },
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::ToByteSlice as _;

    #[test]
    fn test_query_dataset_response_create_dataframe() {
        let chunk_ids = vec![re_chunk::ChunkId::new(), re_chunk::ChunkId::new()];
        let chunk_partition_id = vec!["partition_id_1".to_owned(), "partition_id_2".to_owned()];
        let chunk_layer_names = vec!["layer1".to_owned(), "layer2".to_owned()];
        let chunk_keys = vec![b"key1".to_byte_slice(), b"key2".to_byte_slice()];
        let chunk_entity_paths = vec!["/".to_owned(), "/".to_owned()];
        let chunk_is_static = vec![true, false];

        QueryDatasetResponse::create_dataframe(
            chunk_ids,
            chunk_partition_id,
            chunk_layer_names,
            chunk_keys,
            chunk_entity_paths,
            chunk_is_static,
        )
        .unwrap();
    }

    /// Ensure `crate_dataframe` implementation is consistent with `schema()`
    #[test]
    fn test_scan_partition_table_response_create_dataframe() {
        let partition_ids = vec!["1".to_owned(), "2".to_owned()];
        let layer_names = vec![vec!["a".to_owned(), "b".to_owned()], vec!["c".to_owned()]];
        let storage_urls = vec![vec!["d".to_owned(), "e".to_owned()], vec!["f".to_owned()]];
        let last_updated_at = vec![1, 2];
        let num_chunks = vec![1, 2];
        let size_bytes = vec![1, 2];

        ScanPartitionTableResponse::create_dataframe(
            partition_ids,
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
        let partition_id = vec!["1".to_owned()];
        let storage_url = vec!["d".to_owned()];
        let layer_type = vec!["c".to_owned()];
        let registration_time = vec![1];
        let last_updated_at = vec![2];
        let num_chunks = vec![1];
        let size_bytes = vec![2];
        let schema_sha256 = vec![[1; 32]];

        ScanDatasetManifestResponse::create_dataframe(
            layer_name,
            partition_id,
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
