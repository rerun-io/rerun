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
    pub size_bytes_uncompressed: u64,
}

#[derive(Clone, Copy)]
pub struct ByteFloat(pub f64);

impl std::iter::Sum for ByteFloat {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self(iter.fold(0.0, |acc, item| acc + item.0))
    }
}

impl std::ops::Mul<f32> for ByteFloat {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs as f64)
    }
}

impl std::ops::Div<f32> for ByteFloat {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs as f64)
    }
}

/// In-progress downloads of chunks.
///
/// Used for larger-than-RAM streaming.
pub struct ChunkPromises {
    batches: Vec<ChunkPromiseBatch>,

    pub download_size_history: emath::History<ByteFloat>,
}

impl Default for ChunkPromises {
    fn default() -> Self {
        Self {
            batches: Vec::new(),
            download_size_history: emath::History::new(0..50, 2.0),
        }
    }
}

static_assertions::assert_impl_all!(ChunkPromises: Sync);

#[cfg(feature = "testing")]
impl Clone for ChunkPromises {
    fn clone(&self) -> Self {
        // This means the clone will have to start downloads from scratch.
        // In practice, the `Clone` feature is only used for tests.
        Self {
            batches: Vec::new(),

            download_size_history: {
                let mut h = self.download_size_history.clone();
                h.clear();
                h
            },
        }
    }
}

impl ChunkPromises {
    pub fn has_pending(&self) -> bool {
        !self.batches.is_empty()
    }

    pub fn num_uncompressed_bytes_pending(&self) -> u64 {
        self.batches.iter().map(|b| b.size_bytes_uncompressed).sum()
    }

    /// See if we have received any new chunks since last call.
    pub fn resolve_pending(&mut self, time: f64) -> Vec<Chunk> {
        re_tracing::profile_function!();

        let mut all_chunks = Vec::new();

        let history = &mut self.download_size_history;
        history.flush(time);
        self.batches.retain_mut(|batch| {
            let mut promise_opt = batch.promise.lock();
            if let Some(promise) = promise_opt.take() {
                match promise.try_take() {
                    Ok(Ok(chunks)) => {
                        all_chunks.extend(chunks);
                        history.add(time, ByteFloat(batch.size_bytes_uncompressed as f64));
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
