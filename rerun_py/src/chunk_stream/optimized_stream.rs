//! [`ChunkStreamFactory`] adapter over [`re_chunk_optimizer::optimize`].
//!
//! This backs the private `_optimized_stream()` methods of `LazyStore` and `ChunkStore`.

use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use futures::StreamExt as _;
use futures::stream::BoxStream;
use re_chunk::Chunk;
use re_chunk_optimizer::{MergeSplitSettings, OptimizationSettings};
use re_chunk_store::OptimizationProfile;
use re_log_encoding::ChunkProvider;

use super::error::ChunkPipelineError;
use super::{ChunkStream, ChunkStreamFactory};
use crate::utils::wait_for_future;

/// Resolve the `_optimized_stream()` kwargs into [`OptimizationSettings`].
///
/// Defaults come from [`OptimizationProfile::OBJECT_STORE`]. `chunk_max_bytes == 0` disables the
/// merge/split optimization entirely; passing a row limit with it is an error, because the row
/// guards only exist as guards on the byte target.
//TODO(ab): part of an experimental API to be stabilized and published.
pub fn build_optimization_settings(
    chunk_max_bytes: Option<u64>,
    chunk_max_rows: Option<u64>,
    chunk_max_rows_if_unsorted: Option<u64>,
    target_timeline: Option<String>,
) -> PyResult<OptimizationSettings> {
    let profile = OptimizationProfile::OBJECT_STORE;

    let target_timeline = target_timeline
        .map(|name| {
            re_types_core::TimelineName::try_new(name)
                .map_err(|err| PyValueError::new_err(err.to_string()))
        })
        .transpose()?;

    let max_bytes = chunk_max_bytes.unwrap_or(profile.chunk_max_bytes);
    let merge_split = if let Some(max_bytes) = std::num::NonZeroU64::new(max_bytes) {
        // The kwargs and the profile follow the legacy convention: `0` disables a row guard.
        Some(MergeSplitSettings {
            max_bytes,
            max_rows: std::num::NonZeroU64::new(chunk_max_rows.unwrap_or(profile.chunk_max_rows)),
            max_rows_if_unsorted: std::num::NonZeroU64::new(
                chunk_max_rows_if_unsorted.unwrap_or(profile.chunk_max_rows_if_unsorted),
            ),
        })
    } else {
        if chunk_max_rows.is_some() || chunk_max_rows_if_unsorted.is_some() {
            return Err(PyValueError::new_err(
                "chunk_max_bytes=0 disables the merge/split optimization; \
                 a row limit has no meaning with it",
            ));
        }
        None
    };

    Ok(OptimizationSettings {
        merge_split,
        target_timeline,
    })
}

/// Factory for optimized chunk streams: each `create()` plans and executes the optimization
/// afresh from the provider's chunk index.
pub struct OptimizedStreamFactory {
    pub provider: Arc<dyn ChunkProvider>,
    pub settings: OptimizationSettings,
}

impl ChunkStreamFactory for OptimizedStreamFactory {
    fn create(&self) -> Result<Box<dyn ChunkStream>, ChunkPipelineError> {
        let source = self.provider.source();
        let chunks = re_chunk_optimizer::optimize(Arc::clone(&self.provider), self.settings)
            .map_err(|err| ChunkPipelineError::Optimize {
                from: source.clone(),
                reason: err.to_string(),
            })?
            .boxed();
        Ok(Box::new(OptimizedChunkStream { chunks, source }))
    }
}

/// Pull-based [`ChunkStream`] driving the stream returned by [`re_chunk_optimizer::optimize`].
struct OptimizedChunkStream {
    chunks: BoxStream<'static, Result<Arc<Chunk>, re_chunk_optimizer::Error>>,

    /// The provider's source string, for error messages.
    source: String,
}

impl ChunkStream for OptimizedChunkStream {
    fn next(&mut self) -> Result<Option<Arc<Chunk>>, ChunkPipelineError> {
        Python::attach(|py| wait_for_future(py, self.chunks.next()))
            .transpose()
            .map_err(|err| ChunkPipelineError::Optimize {
                from: self.source.clone(),
                reason: err.to_string(),
            })
    }
}
