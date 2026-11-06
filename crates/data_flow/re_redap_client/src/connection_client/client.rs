use std::sync::Arc;

use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use tokio::sync::OnceCell;

use super::RedapClient;
use crate::extract_trace_id;

/// Extension trait for [`tonic::Response`] that extracts both the inner value
/// and the server's trace-id in one step.
pub trait TonicResponseExt<T> {
    fn into_inner_and_trace_id(self) -> (T, Option<opentelemetry::TraceId>);
}

impl<T> TonicResponseExt<T> for tonic::Response<T> {
    fn into_inner_and_trace_id(self) -> (T, Option<opentelemetry::TraceId>) {
        let trace_id = extract_trace_id(self.metadata());
        (self.into_inner(), trace_id)
    }
}

impl<T> re_byte_size::SizeBytes for RedapClient<T> {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            origin,
            inner: _,
            features,
            chunk_cache,
            schema_cache,
        } = self;

        origin.heap_size_bytes()
            + features.get().map(|v| v.heap_size_bytes()).unwrap_or(0)
            + chunk_cache.heap_size_bytes()
            + schema_cache.heap_size_bytes()
    }
}

impl<T> RedapClient<T> {
    /// Create a new [`Self`].
    ///
    /// The `chunk_cache` is used to cache certain chunks, like from assets. Passing `None`
    /// disables chunk caching.
    ///
    /// Application code should acquire clients through [`crate::ConnectionHandle::client`]
    /// instead.
    pub fn new(
        origin: re_uri::Origin,
        client: RerunCloudServiceClient<T>,
        chunk_cache: Option<crate::ChunkCacheHandle>,
    ) -> Self {
        Self {
            origin,
            inner: client,
            features: Arc::new(OnceCell::new()),
            chunk_cache,
            schema_cache: Default::default(),
        }
    }

    /// The server this client talks to.
    #[inline]
    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    /// Get a mutable reference to the underlying generated gRPC client.
    ///
    /// Callers consuming revision-bearing responses must record them with
    /// [`crate::dataset_revisions()`] before discarding their protobuf metadata.
    //TODO(#10188): this should disappear once we have wrapper for all endpoints and the client code
    //is using them.
    pub fn inner(&mut self) -> &mut RerunCloudServiceClient<T> {
        &mut self.inner
    }

    /// Marks this chunks for caching.
    pub fn mark_asset_chunks(&self, chunk_ids: &[re_chunk::ChunkId]) {
        if let Some(chunk_cache) = &self.chunk_cache {
            chunk_cache.write().mark_chunks_cacheable(chunk_ids);
        }
    }

    /// Drops the cached chunks and schemas, which are fetched again when next needed.
    pub fn purge_memory(&self) {
        if let Some(chunk_cache) = &self.chunk_cache {
            chunk_cache.write().purge_memory();
        }
        self.schema_cache.purge_memory();
    }
}

// `RerunCloudServiceClient<T>`'s derived `Debug` requires `T: Debug`, which doesn't hold for the
// type-erased `BoxedRedapClientStack`. Print only the cached features instead.
impl<T> std::fmt::Debug for RedapClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedapClient")
            .field("features", &self.features)
            .finish_non_exhaustive()
    }
}
