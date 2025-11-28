use re_log_types::EntryId;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_types_core::ChunkId;

use crate::store::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ChunkKey {
    pub chunk_id: ChunkId,
    pub segment_id: SegmentId,
    pub layer_name: String,
    pub dataset_id: EntryId,
}

impl ChunkKey {
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        bincode::serialize(self).map_err(|err| Error::FailedToEncodeChunkKey(err.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        bincode::deserialize(data).map_err(|err| Error::FailedToDecodeChunkKey(err.to_string()))
    }
}
