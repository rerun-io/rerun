use crate::store::Error;
use bincode::config;
use re_log_types::EntryId;
use re_protos::common::v1alpha1::ext::PartitionId;
use re_types_core::ChunkId;
use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkKey {
    pub chunk_id: ChunkId,
    pub partition_id: PartitionId,
    pub layer_name: String,
    pub dataset_id: EntryId,
}

impl ChunkKey {
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        bincode::serde::encode_to_vec(self, config::standard())
            .map_err(|err| Error::FailedToEncodeChunkKey(err.to_string()))
    }

    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        bincode::serde::decode_from_slice(data, config::standard())
            .map(|(key, _bytes_read)| key)
            .map_err(|err| Error::FailedToDecodeChunkKey(err.to_string()))
    }
}
