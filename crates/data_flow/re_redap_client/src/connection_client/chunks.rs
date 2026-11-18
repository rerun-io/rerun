use std::borrow::Cow;

use arrow::array::RecordBatch;
use re_protos::cloud::v1alpha1::FetchChunksRequest;
use re_protos::common::v1alpha1::DataframePart;
use re_protos::external::prost::bytes::Bytes;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::{ApiError, ApiResponseStream, ApiResult};

pub type FetchChunksResponseStream =
    ApiResponseStream<re_protos::cloud::v1alpha1::FetchChunksResponse>;

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    pub fn chunk_cache(&self) -> Option<&crate::ChunkCacheHandle> {
        self.chunk_cache.as_ref()
    }

    /// `record_batch` must carry the columns named in [`FetchChunksRequest::required_column_names`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn fetch_segment_chunks_by_id(
        &mut self,
        record_batch: &RecordBatch,
    ) -> ApiResult<FetchChunksResponseStream> {
        let (cached_chunks, to_fetch) = match &self.chunk_cache {
            Some(chunk_cache) => chunk_cache.split_cached(record_batch),
            None => (Vec::new(), Cow::Borrowed(record_batch)),
        };

        let cached = (!cached_chunks.is_empty()).then(|| {
            Ok(re_protos::cloud::v1alpha1::FetchChunksResponse {
                chunks: cached_chunks,
            })
        });
        let cached_stream = tokio_stream::iter(cached);

        if to_fetch.num_rows() == 0 {
            return Ok(ApiResponseStream::new(
                self.origin.clone(),
                cached_stream,
                None,
            ));
        }

        let fetch_chunks_request = FetchChunksRequest {
            chunk_infos: vec![DataframePart::from(&*to_fetch)],
        };

        let mut req = tonic::Request::new(fetch_chunks_request);
        req.set_timeout(crate::FETCH_CHUNKS_DEADLINE);
        let response = self
            .inner()
            .fetch_chunks(req)
            .await
            // NOTE: `ApiError::tonic` already extracts the trace-id from the error metadata.
            .map_err(|err| ApiError::tonic(&self.origin, err, "/FetchChunks"))?;

        let response =
            ApiResponseStream::from_tonic_response(self.origin.clone(), response, "/FetchChunks");
        let trace_id = response.trace_id();

        let chunk_cache_handle = self.chunk_cache.clone();
        let response = response.map(move |msg| {
            if let Some(chunk_cache) = &chunk_cache_handle
                && let Ok(msg) = &msg
            {
                chunk_cache.insert_cacheable(&msg.chunks);
            }

            msg
        });

        // Order does not have to be preserved, so fine to send cached first.
        Ok(ApiResponseStream::new(
            self.origin.clone(),
            cached_stream.merge(response),
            trace_id,
        ))
    }
}
