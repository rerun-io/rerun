//! Responsible for tracking in-progress chunk downloads for larger-than-RAM.

use parking_lot::Mutex;
use re_chunk::Chunk;

/// A batch of chunks being loaded from a remote server.
pub type ChunkPromise = poll_promise::Promise<Result<Vec<Chunk>, ()>>;

/// Represents a batch of chunks being downloaded.
pub struct ChunkPromiseBatch {
    // The poll_promise API is a bit unergonomic.
    // For one, it is not `Sync`.
    // For another, it is not `Clone`.
    // There is room for something better here at some point.
    pub promise: Mutex<Option<ChunkPromise>>,

    /// Total size of all the chunks in bytes.
    pub size_bytes: u64,
}

/// In-progress downloads of chunks.
///
/// Used for larger-than-RAM streaming.
#[derive(Default)]
pub struct ChunkPromises {
    batches: Vec<ChunkPromiseBatch>,
}

static_assertions::assert_impl_all!(ChunkPromises: Sync);

impl Clone for ChunkPromises {
    fn clone(&self) -> Self {
        // This means the clone will have to start downloads from scratch.
        // In practice, the `Clone` feature is only used for tests.
        Self {
            batches: Vec::new(),
        }
    }
}

impl ChunkPromises {
    pub fn has_pending(&self) -> bool {
        !self.batches.is_empty()
    }

    pub fn num_bytes_pending(&self) -> u64 {
        self.batches.iter().map(|b| b.size_bytes).sum()
    }

    /// See if we have received any new chunks since last call.
    pub fn resolve_pending(&mut self) -> Vec<Chunk> {
        re_tracing::profile_function!();

        let mut all_chunks = Vec::new();

        self.batches.retain_mut(|batch| {
            let mut promise_opt = batch.promise.lock();
            if let Some(promise) = promise_opt.take() {
                match promise.try_take() {
                    Ok(Ok(chunks)) => {
                        all_chunks.extend(chunks);
                        false
                    }
                    Ok(Err(())) => false,
                    Err(promise) => {
                        *promise_opt = Some(promise);
                        true
                    }
                }
            } else {
                false
            }
        });

        all_chunks
    }

    pub fn add(&mut self, batch: ChunkPromiseBatch) {
        self.batches.push(batch);
    }
}
