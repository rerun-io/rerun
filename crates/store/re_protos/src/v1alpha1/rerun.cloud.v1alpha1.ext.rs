use std::sync::Arc;

use arrow::array::RecordBatchOptions;
use arrow::{
    array::{Array, ArrayRef, RecordBatch, StringArray, TimestampNanosecondArray},
    datatypes::{DataType, Field, Schema, TimeUnit},
    error::ArrowError,
};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::TimelineName;
use re_log_types::{EntityPath, EntryId, TimeInt};
use re_sorbet::ComponentColumnDescriptor;

use crate::cloud::v1alpha1::{EntryKind, QueryTasksResponse};
use crate::cloud::v1alpha1::{
    GetDatasetSchemaResponse, RegisterWithDatasetResponse, ScanPartitionTableResponse,
    VectorDistanceMetric,
};
use crate::common::v1alpha1::ext::{
    DatasetHandle, IfDuplicateBehavior, PartitionId, ScanParameters,
};
use crate::common::v1alpha1::{ComponentDescriptor, DataframePart, TaskId};
use crate::{TypeConversionError, missing_field};

// --- ScanPartitionTableRequest ---

pub struct ScanPartitionTableRequest {
    pub dataset_id: EntryId,
    pub scan_parameters: Option<ScanParameters>,
}

impl TryFrom<crate::cloud::v1alpha1::ScanPartitionTableRequest> for ScanPartitionTableRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::ScanPartitionTableRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_id: value
                .dataset_id
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::ScanPartitionTableRequest,
                    "dataset_id"
                ))?
                .try_into()?,
            scan_parameters: value.scan_parameters.map(TryInto::try_into).transpose()?,
        })
    }
}

impl From<ScanPartitionTableRequest> for crate::cloud::v1alpha1::ScanPartitionTableRequest {
    fn from(value: ScanPartitionTableRequest) -> Self {
        Self {
            dataset_id: Some(value.dataset_id.into()),
            scan_parameters: value.scan_parameters.map(Into::into),
        }
    }
}

// --- GetDatasetSchemaRequest ---

impl TryFrom<crate::cloud::v1alpha1::GetDatasetSchemaRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::GetDatasetSchemaRequest,
    ) -> Result<Self, Self::Error> {
        Ok(value
            .dataset_id
            .ok_or(missing_field!(
                crate::cloud::v1alpha1::GetDatasetSchemaRequest,
                "dataset_id"
            ))?
            .try_into()?)
    }
}

// --- RegisterWithDatasetRequest ---

#[derive(Debug)]
pub struct RegisterWithDatasetRequest {
    pub dataset_id: EntryId,
    pub data_sources: Vec<DataSource>,
    pub on_duplicate: IfDuplicateBehavior,
}

impl TryFrom<crate::cloud::v1alpha1::RegisterWithDatasetRequest> for RegisterWithDatasetRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::cloud::v1alpha1::RegisterWithDatasetRequest,
    ) -> Result<Self, Self::Error> {
        let crate::cloud::v1alpha1::RegisterWithDatasetRequest {
            dataset_id,
            data_sources,
            on_duplicate,
        } = value;
        Ok(Self {
            dataset_id: dataset_id
                .ok_or(missing_field!(
                    crate::cloud::v1alpha1::RegisterWithDatasetRequest,
                    "dataset_id"
                ))?
                .try_into()?,
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
            dataset_id: Some(value.dataset_id.into()),
            data_sources: value.data_sources.into_iter().map(Into::into).collect(),
            on_duplicate: crate::common::v1alpha1::IfDuplicateBehavior::from(value.on_duplicate)
                as i32,
        }
    }
}

// --- GetChunksRequest --

#[derive(Debug, Clone)]
pub struct GetChunksRequest {
    pub dataset_id: EntryId,
    pub partition_ids: Vec<crate::common::v1alpha1::ext::PartitionId>,
    pub chunk_ids: Vec<re_chunk::ChunkId>,
    pub entity_paths: Vec<EntityPath>,
    pub query: Option<Query>,
}

impl TryFrom<crate::cloud::v1alpha1::GetChunksRequest> for GetChunksRequest {
    type Error = tonic::Status;

    fn try_from(value: crate::cloud::v1alpha1::GetChunksRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_id: value
                .dataset_id
                .ok_or_else(|| tonic::Status::invalid_argument("dataset_id is required"))?
                .try_into()?,

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

            query: value.query.map(|q| q.try_into()).transpose()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DoMaintenanceRequest {
    pub dataset_id: Option<crate::common::v1alpha1::EntryId>,
    pub build_scalar_indexes: bool,
    pub compact_fragments: bool,
    pub cleanup_before: Option<jiff::Timestamp>,
    pub unsafe_allow_recent_cleanup: bool,
}

impl From<DoMaintenanceRequest> for crate::cloud::v1alpha1::DoMaintenanceRequest {
    fn from(value: DoMaintenanceRequest) -> Self {
        Self {
            dataset_id: value.dataset_id,
            build_scalar_indexes: value.build_scalar_indexes,
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
    pub const TASK_ID: &str = "task_id";
    pub const KIND: &str = "kind";
    pub const DATA: &str = "data";
    pub const EXEC_STATUS: &str = "exec_status";
    pub const MSGS: &str = "msgs";
    pub const BLOB_LEN: &str = "blob_len";
    pub const LEASE_OWNER: &str = "lease_owner";
    pub const LEASE_EXPIRATION: &str = "lease_expiration";
    pub const ATTEMPTS: &str = "attempts";
    pub const CREATION_TIME: &str = "creation_time";
    pub const LAST_UPDATE_TIME: &str = "last_update_time";

    pub fn dataframe_part(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self
            .data
            .as_ref()
            .ok_or_else(|| missing_field!(QueryTasksResponse, "data"))?)
    }

    pub fn schema() -> arrow::datatypes::Schema {
        Schema::new(vec![
            Field::new(Self::TASK_ID, DataType::Utf8, false),
            Field::new(Self::KIND, DataType::Utf8, true),
            Field::new(Self::DATA, DataType::Utf8, true),
            Field::new(Self::EXEC_STATUS, DataType::Utf8, false),
            Field::new(Self::MSGS, DataType::Utf8, true),
            Field::new(Self::BLOB_LEN, DataType::UInt64, true),
            Field::new(Self::LEASE_OWNER, DataType::Utf8, true),
            Field::new(
                Self::LEASE_EXPIRATION,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(Self::ATTEMPTS, DataType::UInt8, false),
            Field::new(
                Self::CREATION_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(
                Self::LAST_UPDATE_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
        ])
    }
}

// --- Catalog ---

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
    pub const PARTITION_ID: &str = "rerun_partition_id";
    pub const PARTITION_TYPE: &str = "rerun_partition_type";
    pub const STORAGE_URL: &str = "rerun_storage_url";
    pub const REGISTRATION_TIME: &str = "rerun_registration_time";
    pub const PARTITION_MANIFEST_UPDATED_AT: &str = "rerun_partition_manifest_updated_at";
    pub const PARTITION_MANIFEST_URL: &str = "rerun_partition_manifest_url";

    pub fn schema() -> Schema {
        Schema::new(vec![
            Field::new(Self::PARTITION_ID, DataType::Utf8, false),
            Field::new(Self::PARTITION_TYPE, DataType::Utf8, false),
            Field::new(Self::STORAGE_URL, DataType::Utf8, false),
            Field::new(
                Self::REGISTRATION_TIME,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                Self::PARTITION_MANIFEST_UPDATED_AT,
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(Self::PARTITION_MANIFEST_URL, DataType::Utf8, true),
        ])
    }

    /// Helper to simplify instantiation of the dataframe in [`Self::data`].
    pub fn create_dataframe(
        partition_ids: Vec<String>,
        partition_types: Vec<String>,
        storage_urls: Vec<String>,
        registration_times: Vec<i64>,
        partition_manifest_updated_ats: Vec<Option<i64>>,
        partition_manifest_urls: Vec<Option<String>>,
    ) -> arrow::error::Result<RecordBatch> {
        let row_count = partition_ids.len();
        let schema = Arc::new(Self::schema());
        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(partition_ids)),
            Arc::new(StringArray::from(partition_types)),
            Arc::new(StringArray::from(storage_urls)),
            Arc::new(TimestampNanosecondArray::from(registration_times)),
            Arc::new(TimestampNanosecondArray::from(
                partition_manifest_updated_ats,
            )),
            Arc::new(StringArray::from(partition_manifest_urls)),
        ];

        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::default().with_row_count(Some(row_count)),
        )
    }

    pub fn data(&self) -> Result<&DataframePart, TypeConversionError> {
        Ok(self.data.as_ref().ok_or_else(|| {
            missing_field!(crate::cloud::v1alpha1::ScanPartitionTableResponse, "data")
        })?)
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
