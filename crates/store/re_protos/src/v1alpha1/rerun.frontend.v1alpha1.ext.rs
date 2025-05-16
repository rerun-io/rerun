use re_log_types::{EntityPath, EntryId};

use crate::v1alpha1::rerun_manifest_registry_v1alpha1_ext::Query;

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
