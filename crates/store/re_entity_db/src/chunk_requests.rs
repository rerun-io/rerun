//! Responsible for tracking in-progress chunk downloads for larger-than-RAM.

use std::collections::BTreeSet;
use std::sync::Arc;

use emath::NumExt as _;
use re_chunk::{Chunk, ChunkId};
use re_mutex::Mutex;

/// A batch of chunks being loaded from a remote server.
pub type ChunkPromise = poll_promise::Promise<Result<Vec<Chunk>, ()>>;

/// Information about a batch of chunks being downloaded.
#[derive(Clone, Debug)]
pub struct RequestInfo {
    /// What chunks are included in this batch.
    pub virtual_chunk_ids: BTreeSet<ChunkId>,

    /// Row indices in the RRD manifest.
    pub row_indices: BTreeSet<usize>,

    /// Total uncompressed size of all chunks in bytes.
    pub size_bytes_uncompressed: u64,

    /// Size on-wire of all chunks in bytes.
    pub size_bytes_on_wire: u64,
}

/// Represents a batch of chunks being downloaded.
pub struct ChunkBatchRequest {
    // The poll_promise API is a bit unergonomic.
    // For one, it is not `Sync`.
    // For another, it is not `Clone`.
    // There is room for something better here at some point.
    pub promise: Mutex<Option<ChunkPromise>>,

    pub info: Arc<RequestInfo>,
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
pub struct ChunkRequests {
    requests: Vec<ChunkBatchRequest>,

    pub download_size_history: emath::History<ByteFloat>,
}

impl Default for ChunkRequests {
    fn default() -> Self {
        Self {
            requests: Vec::new(),
            download_size_history: emath::History::new(0..50, 2.0),
        }
    }
}

static_assertions::assert_impl_all!(ChunkRequests: Sync);

#[cfg(feature = "testing")]
impl Clone for ChunkRequests {
    fn clone(&self) -> Self {
        // This means the clone will have to start downloads from scratch.
        // In practice, the `Clone` feature is only used for tests.
        Self {
            requests: Vec::new(),

            download_size_history: {
                let mut h = self.download_size_history.clone();
                h.clear();
                h
            },
        }
    }
}

impl ChunkRequests {
    pub fn has_pending(&self) -> bool {
        !self.requests.is_empty()
    }

    /// How much data is currently in transit?
    ///
    /// The size is the on-wire size, which is usually
    /// the _compressed_ size.
    pub fn num_on_wire_bytes_pending(&self) -> u64 {
        self.requests
            .iter()
            .map(|b| b.info.size_bytes_uncompressed)
            .sum()
    }

    /// Average of bytes/second over recent history.
    pub fn bandwidth(&self) -> Option<f64> {
        self.download_size_history.bandwidth().map(|b| b.0)
    }

    /// Returns how fresh the bandwidth data is, as a normalized value from 0.0 to 1.0.
    ///
    /// - `1.0` means the most recent download just completed.
    /// - `0.0` means no downloads have completed within `Self.download_size_history.max_age()`.
    pub fn bandwidth_data_freshness(&self, time: f64) -> f64 {
        self.download_size_history
            .iter()
            .last()
            .map(|(t, _)| {
                let age = time - t;

                (1.0 - age / self.download_size_history.max_age() as f64).at_least(0.0)
            })
            .unwrap_or(0.0)
    }

    /// See if we have received any new chunks since last call.
    #[must_use = "Returns newly received chunks"]
    pub fn receive_finished(&mut self, time: f64) -> Vec<Chunk> {
        re_tracing::profile_function!();

        let mut all_chunks = Vec::new();

        let history = &mut self.download_size_history;
        history.flush(time);
        self.requests.retain_mut(|batch| {
            let mut promise_opt = batch.promise.lock();
            if let Some(promise) = promise_opt.take() {
                match promise.try_take() {
                    Ok(Ok(chunks)) => {
                        all_chunks.extend(chunks);
                        history.add(time, ByteFloat(batch.info.size_bytes_on_wire as f64));
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

    pub fn add(&mut self, batch: ChunkBatchRequest) {
        self.requests.push(batch);
    }

    /// Returns info about all in-progress downloads.
    pub fn pending_requests(&self) -> Vec<Arc<RequestInfo>> {
        self.requests
            .iter()
            .map(|batch| Arc::clone(&batch.info))
            .collect()
    }
}
