//! The executor: drives a plan against a [`ChunkProvider`].

pub mod merge_split;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::ChunkProvider;

use crate::Error;
use crate::plan::PlanUnit;
use crate::view::{ChunkIdx, ChunkIndexView};

use merge_split::MergeSplitRunState;

/// Pull-based execution of a plan: chunks are loaded on demand as [`Self::next_chunk`] is driven.
///
/// # Implementation notes
///
/// For now, the executor is a simple state machine that executes one plan unit at a time, with a
/// FIFO queue to hold generated chunks until they are pulled from the stream.
pub struct Executor {
    provider: Arc<dyn ChunkProvider>,
    view: ChunkIndexView,

    /// Previously produced output chunk ready for streaming.
    ready: VecDeque<Arc<Chunk>>,

    /// The remaining plan units to execute, consumed one at a time.
    units: std::vec::IntoIter<PlanUnit>,

    /// State of the in-flight merge/split run, if any.
    run: Option<MergeSplitRunState>,
}

impl Executor {
    pub fn new(
        provider: Arc<dyn ChunkProvider>,
        view: ChunkIndexView,
        units: Vec<PlanUnit>,
    ) -> Self {
        Self {
            provider,
            view,
            units: units.into_iter(),
            run: None,
            ready: VecDeque::new(),
        }
    }

    /// The next optimized chunk, or `None` when done.
    pub async fn next_chunk(&mut self) -> Result<Option<Arc<Chunk>>, Error> {
        loop {
            if let Some(chunk) = self.ready.pop_front() {
                return Ok(Some(chunk));
            }

            if let Some(run) = &mut self.run {
                let flow = run
                    .step(self.provider.as_ref(), &self.view, &mut self.ready)
                    .await?;
                if flow.is_break() {
                    self.run = None;
                }
                continue;
            }

            let Some(output) = self.units.next() else {
                return Ok(None);
            };

            match output {
                PlanUnit::Passthrough(idx) => {
                    let chunks = load_in_order(self.provider.as_ref(), &self.view, &[idx]).await?;
                    self.ready.extend(chunks);
                }

                PlanUnit::MergeSplitRun { inputs, target } => {
                    self.run = Some(MergeSplitRunState::new(inputs, target));
                }
            }
        }
    }
}

/// Load and return the provided chunks in order.
pub async fn load_in_order(
    provider: &dyn ChunkProvider,
    view: &ChunkIndexView,
    idxs: &[ChunkIdx],
) -> Result<Vec<Arc<Chunk>>, Error> {
    let Some(&first) = idxs.first() else {
        return Ok(Vec::new());
    };

    let ids: Vec<ChunkId> = idxs.iter().map(|&idx| view.chunk(idx).chunk_id).collect();

    // Every planned node stays within one entity, so the first chunk names them all.
    let loaded = provider
        .load_chunks(&ids)
        .await
        .map_err(|err| Error::load_chunks(&view.chunk(first).entity_path, ids.len(), err))?;

    let mut by_id: HashMap<ChunkId, Arc<Chunk>> = loaded
        .into_iter()
        .map(|chunk| (chunk.id(), chunk))
        .collect();

    idxs.iter()
        .map(|&idx| {
            let meta = view.chunk(idx);
            by_id
                .remove(&meta.chunk_id)
                .ok_or_else(|| Error::missing_chunk(meta.chunk_id, &meta.entity_path))
        })
        .collect()
}
