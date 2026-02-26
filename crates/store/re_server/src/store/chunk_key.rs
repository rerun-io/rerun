use re_types_core::ChunkId;

use crate::store::Error;
use crate::store::StoreSlotId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ChunkKey {
    pub chunk_id: ChunkId,
    pub store_slot_id: StoreSlotId,
}

impl ChunkKey {
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        bincode::serialize(self).map_err(|err| Error::FailedToEncodeChunkKey(err.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        bincode::deserialize(data).map_err(|err| Error::FailedToDecodeChunkKey(err.to_string()))
    }
}
