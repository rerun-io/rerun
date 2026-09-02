use std::sync::Arc;

use futures::Stream;

use re_chunk::Chunk;
use re_log_encoding::ChunkProvider;

use crate::{Error, OptimizationSettings};

/// Optimize the chunks of a [`ChunkProvider`] into a pull-based stream.
pub fn optimize(
    provider: Arc<dyn ChunkProvider>,
    settings: OptimizationSettings,
) -> Result<impl Stream<Item = Result<Arc<Chunk>, Error>>, Error> {
    let view = crate::view::ChunkIndexView::try_from_raw(provider.raw_manifest())?;
    let units = crate::plan::plan(&view, settings);
    let executor = crate::executor::Executor::new(provider, view, units);

    Ok(futures::stream::try_unfold(
        executor,
        |mut executor| async move {
            let chunk = executor.next_chunk().await?;
            Ok(chunk.map(|chunk| (chunk, executor)))
        },
    ))
}
