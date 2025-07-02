use re_log_types::{EntityPath, EntryId};

use crate::v1alpha1::rerun_common_v1alpha1_ext::{IfDuplicateBehavior, ScanParameters};
use crate::v1alpha1::rerun_manifest_registry_v1alpha1_ext::{DataSource, Query};
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
