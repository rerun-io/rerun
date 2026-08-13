use std::borrow::Cow;
use std::sync::Arc;

use arrow::array::{BooleanArray, RecordBatch};
use arrow::buffer::BooleanBuffer;
use arrow::compute::filter_record_batch;

use re_log_types::external::re_tuid;
use re_protos::log_msg::v1alpha1::ArrowMsg;

/// A shareable handle to a [`ChunkCache`].
#[derive(Clone, Default)]
pub struct ChunkCacheHandle(Arc<re_mutex::RwLock<ChunkCache>>);

impl re_byte_size::SizeBytes for ChunkCacheHandle {
    fn heap_size_bytes(&self) -> u64 {
        let Self(handle) = self;

        handle.read().heap_size_bytes()
    }
}

impl std::fmt::Debug for ChunkCacheHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ChunkCacheHandle").finish()
    }
}

impl ChunkCacheHandle {
    pub fn read(&self) -> re_mutex::RwLockReadGuard<'_, ChunkCache> {
        self.0.read()
    }

    pub(crate) fn write(&self) -> re_mutex::RwLockWriteGuard<'_, ChunkCache> {
        self.0.write()
    }

    /// Splits a chunk request into the chunks this cache can serve and the rows that still have to
    /// be fetched.
    ///
    /// `record_batch` should be shaped like an [`re_log_encoding::RrdManifest`]. It is returned
    /// unchanged when nothing in it is cached.
    pub fn split_cached<'batch>(
        &self,
        record_batch: &'batch RecordBatch,
    ) -> (Vec<ArrowMsg>, Cow<'batch, RecordBatch>) {
        let Some(chunk_ids) = re_log_encoding::RrdManifest::col_chunk_ids_of(record_batch) else {
            re_log::debug_panic!("Failed to read chunk_id field in chunk request");
            return (Vec::new(), Cow::Borrowed(record_batch));
        };

        let cache = self.read();

        // Keep the rows the cache cannot serve.
        let to_fetch = BooleanArray::new(
            BooleanBuffer::collect_bool(chunk_ids.len(), |idx| cache.get(chunk_ids[idx]).is_none()),
            None,
        );

        if to_fetch.false_count() == 0 {
            return (Vec::new(), Cow::Borrowed(record_batch));
        }

        let Ok(to_fetch) = filter_record_batch(record_batch, &to_fetch) else {
            return (Vec::new(), Cow::Borrowed(record_batch));
        };

        // The rows the filter dropped are exactly the ones the cache can serve.
        let cached = chunk_ids
            .iter()
            .filter_map(|chunk_id| cache.get(*chunk_id).cloned())
            .collect();

        (cached, Cow::Owned(to_fetch))
    }

    /// Caches all the chunks in `msgs` that were marked as cacheable.
    pub fn insert_cacheable(&self, msgs: &[ArrowMsg]) {
        let cacheable_chunks = self.0.read().cacheable_chunks_from(msgs);

        // Only take a write lock if there are chunks to cache.
        if !cacheable_chunks.is_empty() {
            let mut cache = self.0.write();

            for chunk in cacheable_chunks {
                cache.insert(chunk);
            }
        }
    }
}

/// Chunks that have already been fetched from a server, keyed by their [`re_chunk::ChunkId`].
///
/// This only caches chunks that have been marked as cacheable.
#[derive(Clone, Default, re_byte_size::SizeBytes)]
pub struct ChunkCache {
    /// The cached transport level messages.
    ///
    /// `None` means the given chunk should be cached, but hasn't been downloaded yet.
    chunks: ahash::HashMap<re_chunk::ChunkId, Option<ArrowMsg>>,
}

impl ChunkCache {
    pub fn get(&self, id: re_chunk::ChunkId) -> Option<&ArrowMsg> {
        self.chunks.get(&id)?.as_ref()
    }

    // TODO(isse): Could we track this more granularly?
    // Where we need to check if chunks should be cached we:
    // - Have access to the `SegmentId`, but that isn't unique across datasets.
    // - Don't have access to what dataset a chunk is from.
    // - Can't check a chunks lineage.
    // - Not ideal to add more information to the record batch, given that's
    //   what's sent over the wire.
    /// Marks that the given chunks should be cached when downloaded.
    ///
    /// Chunks that are already cached keep their data, since the same asset is marked again every
    /// time a segment that uses it is opened.
    pub fn mark_chunks_cacheable(&mut self, chunk_ids: &[re_chunk::ChunkId]) {
        for chunk_id in chunk_ids {
            self.chunks.entry(*chunk_id).or_default();
        }
    }

    fn cacheable_chunks_from<'a>(&self, msgs: &'a [ArrowMsg]) -> Vec<&'a ArrowMsg> {
        msgs.iter()
            .filter(|msg| {
                let Some(chunk_id) = msg
                    .chunk_id
                    .and_then(|tuid| re_tuid::Tuid::try_from(tuid).ok())
                else {
                    return false;
                };
                let chunk_id = re_chunk::ChunkId::from_tuid(chunk_id);

                self.chunks.contains_key(&chunk_id)
            })
            .collect()
    }

    /// Caches a chunk, if it belongs to an asset.
    fn insert(&mut self, msg: &ArrowMsg) {
        let Some(chunk_id) = msg
            .chunk_id
            .and_then(|tuid| re_tuid::Tuid::try_from(tuid).ok())
        else {
            return;
        };
        let chunk_id = re_chunk::ChunkId::from_tuid(chunk_id);

        // The chunk was marked cacheable, so its entry is already there waiting to be filled.
        if let Some(cached) = self.chunks.get_mut(&chunk_id) {
            cached.get_or_insert_with(|| msg.clone());
        }
    }

    /// Drops all cached chunks.
    ///
    /// The chunk ids marked as cacheable are kept, so that asset chunks are cached again as they
    /// come back over the wire.
    pub fn purge_memory(&mut self) {
        #[expect(clippy::iter_over_hash_type)] // Sets all values to None.
        for chunk in self.chunks.values_mut() {
            *chunk = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use arrow::array::Array as _;

    use super::*;

    fn arrow_msg(chunk_id: re_chunk::ChunkId, payload: &'static [u8]) -> ArrowMsg {
        ArrowMsg {
            chunk_id: Some(chunk_id.as_tuid().into()),
            payload: tonic::codegen::Bytes::from_static(payload),
            ..Default::default()
        }
    }

    fn payload_of(cache: &ChunkCache, chunk_id: re_chunk::ChunkId) -> Option<&[u8]> {
        cache.get(chunk_id).map(|msg| &*msg.payload)
    }

    /// A chunk request covering `chunk_ids`, shaped like
    /// [`re_log_encoding::RrdManifest::chunk_fetcher_rb`].
    fn chunk_request(chunk_ids: &[re_chunk::ChunkId]) -> RecordBatch {
        let column = re_chunk::ChunkId::arrow_from_slice(chunk_ids);
        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![arrow::datatypes::Field::new(
                re_log_encoding::RrdManifest::FIELD_CHUNK_ID,
                column.data_type().clone(),
                false,
            )],
            Default::default(),
        );

        RecordBatch::try_new_with_options(
            Arc::new(schema),
            vec![Arc::new(column)],
            &Default::default(),
        )
        .expect("the column matches the schema")
    }

    fn chunk_ids_of(record_batch: &RecordBatch) -> Vec<re_chunk::ChunkId> {
        re_log_encoding::RrdManifest::col_chunk_ids_of(record_batch)
            .expect("the request has a chunk id column")
            .to_vec()
    }

    /// Chunks for a marked and an unmarked chunk id arrive together. Only the marked one is kept.
    #[test]
    fn only_marked_chunks_are_cached() {
        let marked = re_chunk::ChunkId::new();
        let unmarked = re_chunk::ChunkId::new();

        let cache = ChunkCacheHandle::default();
        cache.write().mark_chunks_cacheable(&[marked]);
        cache.insert_cacheable(&[
            arrow_msg(marked, b"asset"),
            arrow_msg(unmarked, b"recording"),
        ]);

        assert_eq!(payload_of(&cache.read(), marked), Some(&b"asset"[..]));
        assert_eq!(payload_of(&cache.read(), unmarked), None);
    }

    /// An asset is marked again every time a segment that uses it is opened. Chunks downloaded for
    /// an earlier segment stay cached across that.
    #[test]
    fn marking_a_cached_chunk_again_keeps_it() {
        let chunk_id = re_chunk::ChunkId::new();

        let cache = ChunkCacheHandle::default();
        cache.write().mark_chunks_cacheable(&[chunk_id]);
        cache.insert_cacheable(&[arrow_msg(chunk_id, b"asset")]);

        cache.write().mark_chunks_cacheable(&[chunk_id]);

        assert_eq!(payload_of(&cache.read(), chunk_id), Some(&b"asset"[..]));
    }

    /// Purging drops the data but keeps the chunk marked, so it is cached again the next time it
    /// comes over the wire.
    #[test]
    fn a_purged_chunk_is_cached_again_when_it_comes_back() {
        let chunk_id = re_chunk::ChunkId::new();

        let cache = ChunkCacheHandle::default();
        cache.write().mark_chunks_cacheable(&[chunk_id]);
        cache.insert_cacheable(&[arrow_msg(chunk_id, b"asset")]);

        cache.write().purge_memory();
        assert_eq!(payload_of(&cache.read(), chunk_id), None);

        cache.insert_cacheable(&[arrow_msg(chunk_id, b"asset")]);
        assert_eq!(payload_of(&cache.read(), chunk_id), Some(&b"asset"[..]));
    }

    /// A request covers one cached and one uncached chunk. The cached one comes back as a chunk,
    /// and only the uncached row is left to fetch.
    #[test]
    fn splitting_a_request_leaves_only_the_uncached_rows() {
        let cached_id = re_chunk::ChunkId::new();
        let uncached_id = re_chunk::ChunkId::new();

        let cache = ChunkCacheHandle::default();
        cache.write().mark_chunks_cacheable(&[cached_id]);
        cache.insert_cacheable(&[arrow_msg(cached_id, b"asset")]);

        let request = chunk_request(&[cached_id, uncached_id]);
        let (cached, to_fetch) = cache.split_cached(&request);

        assert_eq!(cached.len(), 1);
        assert_eq!(&*cached[0].payload, b"asset");
        assert_eq!(chunk_ids_of(&to_fetch), vec![uncached_id]);
    }

    /// A request with nothing cached is passed along untouched.
    #[test]
    fn splitting_a_request_with_nothing_cached_keeps_every_row() {
        let chunk_ids = [re_chunk::ChunkId::new(), re_chunk::ChunkId::new()];

        let cache = ChunkCacheHandle::default();
        let request = chunk_request(&chunk_ids);
        let (cached, to_fetch) = cache.split_cached(&request);

        assert!(cached.is_empty());
        assert_eq!(chunk_ids_of(&to_fetch), chunk_ids);
    }

    /// A fully cached request leaves nothing to ask the server for.
    #[test]
    fn splitting_a_fully_cached_request_leaves_no_rows() {
        let chunk_ids = [re_chunk::ChunkId::new(), re_chunk::ChunkId::new()];

        let cache = ChunkCacheHandle::default();
        cache.write().mark_chunks_cacheable(&chunk_ids);
        cache.insert_cacheable(&[
            arrow_msg(chunk_ids[0], b"asset"),
            arrow_msg(chunk_ids[1], b"asset"),
        ]);

        let request = chunk_request(&chunk_ids);
        let (cached, to_fetch) = cache.split_cached(&request);

        assert_eq!(cached.len(), 2);
        assert_eq!(to_fetch.num_rows(), 0);
    }
}
