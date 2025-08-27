use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

use re_log_types::{EntityPath, EntryId};

use crate::common::v1alpha1::DataframePart;
use crate::common::v1alpha1::ext::{
    DatasetHandle, IfDuplicateBehavior, PartitionId, ScanParameters,
};
use crate::frontend::v1alpha1::{EntryKind, QueryTasksResponse};
use crate::manifest_registry::v1alpha1::ext::{DataSource, Query};
use crate::{TypeConversionError, missing_field};

// --- GetPartitionTableSchemaRequest ---

impl TryFrom<crate::frontend::v1alpha1::GetPartitionTableSchemaRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::GetPartitionTableSchemaRequest,
    ) -> Result<Self, Self::Error> {
        Ok(value
            .dataset_id
            .ok_or(missing_field!(
                crate::frontend::v1alpha1::GetPartitionTableSchemaRequest,
                "dataset_id"
            ))?
            .try_into()?)
    }
}

// --- ScanPartitionTableRequest ---

pub struct ScanPartitionTableRequest {
    pub dataset_id: EntryId,
    pub scan_parameters: Option<ScanParameters>,
}

impl TryFrom<crate::frontend::v1alpha1::ScanPartitionTableRequest> for ScanPartitionTableRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::ScanPartitionTableRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_id: value
                .dataset_id
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::ScanPartitionTableRequest,
                    "dataset_id"
                ))?
                .try_into()?,
            scan_parameters: value.scan_parameters.map(TryInto::try_into).transpose()?,
        })
    }
}

impl From<ScanPartitionTableRequest> for crate::frontend::v1alpha1::ScanPartitionTableRequest {
    fn from(value: ScanPartitionTableRequest) -> Self {
        Self {
            dataset_id: Some(value.dataset_id.into()),
            scan_parameters: value.scan_parameters.map(Into::into),
        }
    }
}

// --- GetDatasetSchemaRequest ---

impl TryFrom<crate::frontend::v1alpha1::GetDatasetSchemaRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::GetDatasetSchemaRequest,
    ) -> Result<Self, Self::Error> {
        Ok(value
            .dataset_id
            .ok_or(missing_field!(
                crate::frontend::v1alpha1::GetDatasetSchemaRequest,
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

impl TryFrom<crate::frontend::v1alpha1::RegisterWithDatasetRequest> for RegisterWithDatasetRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::RegisterWithDatasetRequest,
    ) -> Result<Self, Self::Error> {
        let crate::frontend::v1alpha1::RegisterWithDatasetRequest {
            dataset_id,
            data_sources,
            on_duplicate,
        } = value;
        Ok(Self {
            dataset_id: dataset_id
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::RegisterWithDatasetRequest,
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

impl From<RegisterWithDatasetRequest> for crate::frontend::v1alpha1::RegisterWithDatasetRequest {
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

impl TryFrom<crate::frontend::v1alpha1::GetChunksRequest> for GetChunksRequest {
    type Error = tonic::Status;

    fn try_from(value: crate::frontend::v1alpha1::GetChunksRequest) -> Result<Self, Self::Error> {
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

impl From<DoMaintenanceRequest> for crate::frontend::v1alpha1::DoMaintenanceRequest {
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

impl crate::frontend::v1alpha1::EntryFilter {
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
    pub kind: crate::frontend::v1alpha1::EntryKind,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}

impl TryFrom<crate::frontend::v1alpha1::EntryDetails> for EntryDetails {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::EntryDetails) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::EntryDetails,
                    "id"
                ))?
                .try_into()?,
            name: value.name.ok_or(missing_field!(
                crate::frontend::v1alpha1::EntryDetails,
                "name"
            ))?,
            kind: value.entry_kind.try_into()?,
            created_at: {
                let ts = value.created_at.ok_or(missing_field!(
                    crate::frontend::v1alpha1::EntryDetails,
                    "created_at"
                ))?;
                jiff::Timestamp::new(ts.seconds, ts.nanos)?
            },
            updated_at: {
                let ts = value.updated_at.ok_or(missing_field!(
                    crate::frontend::v1alpha1::EntryDetails,
                    "updated_at"
                ))?;
                jiff::Timestamp::new(ts.seconds, ts.nanos)?
            },
        })
    }
}

impl From<EntryDetails> for crate::frontend::v1alpha1::EntryDetails {
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

impl TryFrom<crate::frontend::v1alpha1::DatasetDetails> for DatasetDetails {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::DatasetDetails) -> Result<Self, Self::Error> {
        Ok(Self {
            blueprint_dataset: value.blueprint_dataset.map(TryInto::try_into).transpose()?,
            default_blueprint: value.default_blueprint.map(TryInto::try_into).transpose()?,
        })
    }
}

impl From<DatasetDetails> for crate::frontend::v1alpha1::DatasetDetails {
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

impl TryFrom<crate::frontend::v1alpha1::DatasetEntry> for DatasetEntry {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::DatasetEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            details: value
                .details
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::DatasetEntry,
                    "details"
                ))?
                .try_into()?,
            dataset_details: value
                .dataset_details
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::DatasetDetails,
                    "dataset_details"
                ))?
                .try_into()?,
            handle: value
                .dataset_handle
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::DatasetEntry,
                    "handle"
                ))?
                .try_into()?,
        })
    }
}

impl From<DatasetEntry> for crate::frontend::v1alpha1::DatasetEntry {
    fn from(value: DatasetEntry) -> Self {
        Self {
            details: Some(value.details.into()),
            dataset_details: Some(value.dataset_details.into()),
            dataset_handle: Some(value.handle.into()),
        }
    }
}

// --- CreateDatasetEntryRequest ---

impl TryFrom<crate::frontend::v1alpha1::CreateDatasetEntryRequest> for String {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::CreateDatasetEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(value.name.ok_or(missing_field!(
            crate::frontend::v1alpha1::CreateDatasetEntryRequest,
            "name"
        ))?)
    }
}

// --- CreateDatasetEntryResponse ---

#[derive(Debug, Clone)]
pub struct CreateDatasetEntryResponse {
    pub dataset: DatasetEntry,
}

impl From<CreateDatasetEntryResponse> for crate::frontend::v1alpha1::CreateDatasetEntryResponse {
    fn from(value: CreateDatasetEntryResponse) -> Self {
        Self {
            dataset: Some(value.dataset.into()),
        }
    }
}

impl TryFrom<crate::frontend::v1alpha1::CreateDatasetEntryResponse> for CreateDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::CreateDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset: value
                .dataset
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::CreateDatasetEntryResponse,
                    "dataset"
                ))?
                .try_into()?,
        })
    }
}

// --- ReadDatasetEntryRequest ---

impl TryFrom<crate::frontend::v1alpha1::ReadDatasetEntryRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::ReadDatasetEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(value
            .id
            .ok_or(missing_field!(
                crate::frontend::v1alpha1::ReadDatasetEntryRequest,
                "id"
            ))?
            .try_into()?)
    }
}

// --- ReadDatasetEntryResponse ---

#[derive(Debug, Clone)]
pub struct ReadDatasetEntryResponse {
    pub dataset_entry: DatasetEntry,
}

impl From<ReadDatasetEntryResponse> for crate::frontend::v1alpha1::ReadDatasetEntryResponse {
    fn from(value: ReadDatasetEntryResponse) -> Self {
        Self {
            dataset: Some(value.dataset_entry.into()),
        }
    }
}

impl TryFrom<crate::frontend::v1alpha1::ReadDatasetEntryResponse> for ReadDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::ReadDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_entry: value
                .dataset
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::ReadDatasetEntryResponse,
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

impl TryFrom<crate::frontend::v1alpha1::UpdateDatasetEntryRequest> for UpdateDatasetEntryRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::UpdateDatasetEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::UpdateDatasetEntryRequest,
                    "id"
                ))?
                .try_into()?,
            dataset_details: value
                .dataset_details
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::UpdateDatasetEntryRequest,
                    "dataset_details"
                ))?
                .try_into()?,
        })
    }
}

impl From<UpdateDatasetEntryRequest> for crate::frontend::v1alpha1::UpdateDatasetEntryRequest {
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

impl From<UpdateDatasetEntryResponse> for crate::frontend::v1alpha1::UpdateDatasetEntryResponse {
    fn from(value: UpdateDatasetEntryResponse) -> Self {
        Self {
            dataset: Some(value.dataset_entry.into()),
        }
    }
}

impl TryFrom<crate::frontend::v1alpha1::UpdateDatasetEntryResponse> for UpdateDatasetEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::UpdateDatasetEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            dataset_entry: value
                .dataset
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::UpdateDatasetEntryResponse,
                    "dataset"
                ))?
                .try_into()?,
        })
    }
}

// --- DeleteEntryRequest ---

impl TryFrom<crate::frontend::v1alpha1::DeleteEntryRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::DeleteEntryRequest) -> Result<Self, Self::Error> {
        Ok(value
            .id
            .ok_or(missing_field!(
                crate::frontend::v1alpha1::DeleteEntryRequest,
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

impl TryFrom<crate::frontend::v1alpha1::EntryDetailsUpdate> for EntryDetailsUpdate {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::EntryDetailsUpdate) -> Result<Self, Self::Error> {
        Ok(Self { name: value.name })
    }
}

impl From<EntryDetailsUpdate> for crate::frontend::v1alpha1::EntryDetailsUpdate {
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

impl TryFrom<crate::frontend::v1alpha1::UpdateEntryRequest> for UpdateEntryRequest {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::UpdateEntryRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::UpdateEntryRequest,
                    "id"
                ))?
                .try_into()?,
            entry_details_update: value
                .entry_details_update
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::UpdateEntryRequest,
                    "entry_details_update"
                ))?
                .try_into()?,
        })
    }
}

impl From<UpdateEntryRequest> for crate::frontend::v1alpha1::UpdateEntryRequest {
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

impl TryFrom<crate::frontend::v1alpha1::UpdateEntryResponse> for UpdateEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::UpdateEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            entry_details: value
                .entry_details
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::UpdateEntryResponse,
                    "entry_details"
                ))?
                .try_into()?,
        })
    }
}

impl From<UpdateEntryResponse> for crate::frontend::v1alpha1::UpdateEntryResponse {
    fn from(value: UpdateEntryResponse) -> Self {
        Self {
            entry_details: Some(value.entry_details.into()),
        }
    }
}

// --- ReadTableEntryRequest ---

impl TryFrom<crate::frontend::v1alpha1::ReadTableEntryRequest> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::ReadTableEntryRequest,
    ) -> Result<Self, Self::Error> {
        Ok(value
            .id
            .ok_or(missing_field!(
                crate::frontend::v1alpha1::ReadTableEntryRequest,
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

impl From<ReadTableEntryResponse> for crate::frontend::v1alpha1::ReadTableEntryResponse {
    fn from(value: ReadTableEntryResponse) -> Self {
        Self {
            table: Some(value.table_entry.into()),
        }
    }
}

impl TryFrom<crate::frontend::v1alpha1::ReadTableEntryResponse> for ReadTableEntryResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::ReadTableEntryResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            table_entry: value
                .table
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::ReadTableEntryResponse,
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

impl From<RegisterTableRequest> for crate::frontend::v1alpha1::RegisterTableRequest {
    fn from(value: RegisterTableRequest) -> Self {
        Self {
            name: value.name,
            provider_details: Some(value.provider_details),
        }
    }
}

impl TryFrom<crate::frontend::v1alpha1::RegisterTableRequest> for RegisterTableRequest {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::RegisterTableRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            provider_details: value.provider_details.ok_or(missing_field!(
                crate::frontend::v1alpha1::RegisterTableRequest,
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

impl TryFrom<crate::frontend::v1alpha1::RegisterTableResponse> for RegisterTableResponse {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::frontend::v1alpha1::RegisterTableResponse,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            table_entry: value
                .table_entry
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::RegisterTableResponse,
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

impl From<TableEntry> for crate::frontend::v1alpha1::TableEntry {
    fn from(value: TableEntry) -> Self {
        Self {
            details: Some(value.details.into()),
            provider_details: Some(value.provider_details),
        }
    }
}

impl TryFrom<crate::frontend::v1alpha1::TableEntry> for TableEntry {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::TableEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            details: value
                .details
                .ok_or(missing_field!(
                    crate::frontend::v1alpha1::TableEntry,
                    "details"
                ))?
                .try_into()?,
            provider_details: value.provider_details.ok_or(missing_field!(
                crate::frontend::v1alpha1::TableEntry,
                "handle"
            ))?,
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
    pub kind: crate::frontend::v1alpha1::SystemTableKind,
}

impl TryFrom<crate::frontend::v1alpha1::SystemTable> for SystemTable {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::SystemTable) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: value.kind.try_into()?,
        })
    }
}

impl From<SystemTable> for crate::frontend::v1alpha1::SystemTable {
    fn from(value: SystemTable) -> Self {
        Self {
            kind: value.kind as _,
        }
    }
}

impl ProviderDetails for SystemTable {
    fn try_as_any(&self) -> Result<prost_types::Any, TypeConversionError> {
        let as_proto: crate::frontend::v1alpha1::SystemTable = self.clone().into();
        Ok(prost_types::Any::from_msg(&as_proto)?)
    }

    fn try_from_any(any: &prost_types::Any) -> Result<Self, TypeConversionError> {
        let as_proto = any.to_msg::<crate::frontend::v1alpha1::SystemTable>()?;
        Ok(as_proto.try_into()?)
    }
}

// --- LanceTable ---

#[derive(Debug, Clone)]
pub struct LanceTable {
    pub table_url: url::Url,
}

impl TryFrom<crate::frontend::v1alpha1::LanceTable> for LanceTable {
    type Error = TypeConversionError;

    fn try_from(value: crate::frontend::v1alpha1::LanceTable) -> Result<Self, Self::Error> {
        Ok(Self {
            table_url: url::Url::parse(&value.table_url)?,
        })
    }
}

impl From<LanceTable> for crate::frontend::v1alpha1::LanceTable {
    fn from(value: LanceTable) -> Self {
        Self {
            table_url: value.table_url.to_string(),
        }
    }
}

impl ProviderDetails for LanceTable {
    fn try_as_any(&self) -> Result<prost_types::Any, TypeConversionError> {
        let as_proto: crate::frontend::v1alpha1::LanceTable = self.clone().into();
        Ok(prost_types::Any::from_msg(&as_proto)?)
    }

    fn try_from_any(any: &prost_types::Any) -> Result<Self, TypeConversionError> {
        let as_proto = any.to_msg::<crate::frontend::v1alpha1::LanceTable>()?;
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
