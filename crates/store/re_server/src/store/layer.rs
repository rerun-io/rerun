use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreHandle;

#[derive(Clone)]
pub struct Layer {
    store_handle: ChunkStoreHandle,

    #[expect(dead_code)]
    registration_time: jiff::Timestamp,
}

impl Layer {
    pub fn new(store_handle: ChunkStoreHandle) -> Self {
        store_handle.into()
    }

    pub fn store_handle(&self) -> &ChunkStoreHandle {
        &self.store_handle
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
}

impl From<ChunkStoreHandle> for Layer {
    fn from(value: ChunkStoreHandle) -> Self {
        Self {
            store_handle: value,
            registration_time: jiff::Timestamp::now(),
        }
    }
}
