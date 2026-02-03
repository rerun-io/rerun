use arrow::array::RecordBatch;
use arrow::datatypes::Schema;
use arrow::error::ArrowError;
use re_byte_size::SizeBytes as _;
use re_chunk_store::ChunkStoreHandle;
use re_log_types::{AbsoluteTimeRange, Timeline};
use std::collections::{BTreeMap, HashMap};

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
        self.store_handle.read().num_physical_chunks() as u64
    }

    pub fn size_bytes(&self) -> u64 {
        self.store_handle
            .read()
            .iter_physical_chunks()
            .map(|chunk| chunk.heap_size_bytes())
            .sum()
    }

    pub fn schema(&self) -> Schema {
        let fields = self.store_handle.read().schema().arrow_fields();
        Schema::new_with_metadata(fields, HashMap::default())
    }

    pub fn schema_sha256(&self) -> Result<[u8; 32], ArrowError> {
        re_log_encoding::RawRrdManifest::compute_sorbet_schema_sha256(&self.schema())
    }

    pub fn compute_properties(
        &self,
    ) -> Result<RecordBatch, re_chunk_store::ExtractPropertiesError> {
        self.store_handle.read().extract_properties()
    }

    pub fn index_ranges(&self) -> BTreeMap<Timeline, AbsoluteTimeRange> {
        let mut ranges = BTreeMap::new();
        for chunk in self.store_handle.read().iter_physical_chunks() {
            for time_col in chunk.timelines().values() {
                let timeline = time_col.timeline().to_owned();
                let range = time_col.time_range();

                let entry = ranges.entry(timeline).or_insert(range);
                *entry = entry.union(range);
            }
        }

        ranges
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
