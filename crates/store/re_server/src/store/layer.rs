use std::collections::HashMap;

use arrow::datatypes::Schema;
use arrow::error::ArrowError;
use sha2::Digest as _;

use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreHandle;

#[derive(Clone)]
pub struct Layer {
    store_handle: ChunkStoreHandle,
    registration_time: jiff::Timestamp,
}

impl Layer {
    pub fn new(store_handle: ChunkStoreHandle) -> Self {
        store_handle.into()
    }

    pub fn store_handle(&self) -> &ChunkStoreHandle {
        &self.store_handle
    }

    pub fn registration_time(&self) -> jiff::Timestamp {
        self.registration_time
    }

    pub fn last_updated_at(&self) -> jiff::Timestamp {
        //TODO(ab): change this if we ever mutate a layer somehow?
        self.registration_time
    }

    #[expect(clippy::unused_self)]
    pub fn layer_type(&self) -> &'static str {
        //TODO(ab): what should that actually be?
        "rrd"
    }

    pub fn num_chunks(&self) -> u64 {
        self.store_handle.read().num_chunks() as u64
    }

    pub fn size_bytes(&self) -> u64 {
        self.store_handle
            .read()
            .iter_chunks()
            .map(|chunk| chunk.heap_size_bytes())
            .sum()
    }

    pub fn schema(&self) -> Schema {
        let fields = self.store_handle.read().schema().arrow_fields();
        Schema::new_with_metadata(fields, HashMap::default())
    }

    pub fn schema_sha256(&self) -> Result<[u8; 32], ArrowError> {
        let schema = {
            // Sort and remove top-level metadata before hashing.
            let mut fields = self.schema().fields().to_vec();
            fields.sort();
            Schema::new_with_metadata(fields, Default::default()) // no metadata!
        };

        let partition_schema_ipc = {
            let mut schema_ipc = Vec::new();
            arrow::ipc::writer::StreamWriter::try_new(&mut schema_ipc, &schema)?;
            schema_ipc
        };

        let mut hash = [0u8; 32];
        let mut hasher = sha2::Sha256::new();
        hasher.update(&partition_schema_ipc);
        hasher.finalize_into(sha2::digest::generic_array::GenericArray::from_mut_slice(
            &mut hash,
        ));

        Ok(hash)
    }
}

impl From<ChunkStoreHandle> for Layer {
    fn from(value: ChunkStoreHandle) -> Self {
        Self {
            store_handle: value,
            registration_time: jiff::Timestamp::now(),
        }
    }
}
