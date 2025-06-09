use emath::History;
use parking_lot::Mutex;

use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent};
use re_log_types::StoreId;
use re_sorbet::timing_metadata::TimestampMetadata;

/// Statistics about the latency of incoming data to a store.
pub struct IngestionStatistics {
    store_id: StoreId,
    stats: Mutex<LatencyStats>,
}

impl IngestionStatistics {
    #[inline]
    pub fn on_events(&self, timestamps: &TimestampMetadata, events: &[ChunkStoreEvent]) {
        if let Some(nanos_since_epoch) = nanos_since_epoch() {
            let mut stats = self.stats.lock();
            for event in events {
                if event.store_id == self.store_id
                    && event.diff.kind == ChunkStoreDiffKind::Addition
                {
                    stats.on_new_chunk(nanos_since_epoch, timestamps, &event.diff.chunk);
                }
            }
        }
    }
}

impl IngestionStatistics {
    pub fn new(store_id: StoreId) -> Self {
        Self {
            store_id,
            stats: Default::default(),
        }
    }

    /// The latest (smoothed) reading of the latency of the ingestion pipeline.
    ///
    /// Returns `None` if we don't have enough data to compute a meaningful average.
    pub fn latency_snapshot(&self) -> Option<LatencySnapshot> {
        self.stats.lock().snapshot()
    }
}

/// Statistics about the latency of incoming data to a store.
#[derive(Clone, Debug)]
pub struct LatencyStats {
    // All latencies measured in seconds.
    /// Full end-to-end latency, from the time the data was logged in the SDK,
    /// up until it was added to the store.
    e2e: History<f32>,

    /// Delay between the time `RowId` was created and the `ChunkId` was created,
    /// i.e. the time it took to get the data from the `log` call to be batched by the batcher.
    log2chunk: History<f32>,

    /// Time between chunk creation and IPC encoding (start of gRPC transmission).
    chunk2encode: History<f32>,

    /// Time between encoding to IPC and decoding again,
    /// e.g. the time it takes to send the data over the network.
    transmission: History<f32>,
}

impl Default for LatencyStats {
    fn default() -> Self {
        let min_samples = 0; // 0: we stop displaying e2e latency if input stops
        let max_samples = 32 * 1024; // don't waste too much memory on this - we just need enough to get a good average
        let max_age = 1.0; // don't keep too long of a rolling average, or the stats get outdated.
        Self {
            e2e: History::new(min_samples..max_samples, max_age),
            log2chunk: History::new(min_samples..max_samples, max_age),
            chunk2encode: History::new(min_samples..max_samples, max_age),
            transmission: History::new(min_samples..max_samples, max_age),
        }
    }
}

impl LatencyStats {
    fn on_new_chunk(
        &mut self,
        nanos_since_epoch: u64,
        timestamps: &TimestampMetadata,
        chunk: &re_chunk::Chunk,
    ) {
        let Self {
            e2e,
            log2chunk,
            chunk2encode,
            transmission,
        } = self;

        let now = nanos_since_epoch as f64 / 1e9;

        for row_id in chunk.row_ids() {
            if let Some(nanos_since_log) = nanos_since_epoch.checked_sub(row_id.nanos_since_epoch())
            {
                let now = nanos_since_epoch as f64 / 1e9;
                let sec_since_log = nanos_since_log as f32 / 1e9;

                e2e.add(now, sec_since_log);
            }

            if let Some(log2chunk_nanos) = chunk
                .id()
                .nanos_since_epoch()
                .checked_sub(row_id.nanos_since_epoch())
            {
                let log2chunk_sec = log2chunk_nanos as f32 / 1e9;
                log2chunk.add(now, log2chunk_sec);
            }

            let TimestampMetadata {
                last_encoded_at,
                last_decoded_at,
            } = timestamps;

            let last_encoded_at_nanos = last_encoded_at.and_then(system_time_to_nanos);
            let last_decoded_at_nanos = last_decoded_at.and_then(system_time_to_nanos);

            if let Some(last_encoded_at_nanos) = last_encoded_at_nanos {
                chunk2encode.add(
                    now,
                    (last_encoded_at_nanos as i64 - row_id.nanos_since_epoch() as i64) as f32 / 1e9,
                );

                if let Some(last_decoded_at_nanos) = last_decoded_at_nanos {
                    transmission.add(
                        now,
                        (last_decoded_at_nanos as i64 - last_encoded_at_nanos as i64) as f32 / 1e9,
                    );
                }
            }
        }
    }

    /// What is the mean latency between the time data was logged in the SDK and the time it was ingested?
    ///
    /// This is based on the clocks of the viewer and the SDK being in sync,
    /// so if the recording was done on another machine, this is likely very inaccurate.
    ///
    /// Returns `None` if we don't have enough data to compute a meaningful average.
    pub fn snapshot(&mut self) -> Option<LatencySnapshot> {
        let Self {
            e2e,
            log2chunk,
            chunk2encode,
            transmission,
        } = self;

        if let Some(nanos_since_epoch) = nanos_since_epoch() {
            let now = nanos_since_epoch as f64 / 1e9;
            // make sure the average is up-to-date.:
            e2e.flush(now);
            log2chunk.flush(now);
            chunk2encode.flush(now);
            transmission.flush(now);
        }

        Some(LatencySnapshot {
            e2e: e2e.average()?,
            log2chunk: log2chunk.average()?,
            chunk2encode: chunk2encode.average(),
            transmission: transmission.average(),
        })
    }
}

fn nanos_since_epoch() -> Option<u64> {
    if let Ok(duration_since_epoch) = web_time::SystemTime::UNIX_EPOCH.elapsed() {
        Some(duration_since_epoch.as_nanos() as u64)
    } else {
        None
    }
}

fn system_time_to_nanos(system_time: web_time::SystemTime) -> Option<u64> {
    system_time
        .duration_since(web_time::SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos() as u64)
}

/// The latest (smoothed) reading of the latency of the ingestion pipeline.
///
/// All measurements are in seconds, and are only valid if the clocks of the viewer and the SDK are in sync.
#[derive(Clone, Copy, Debug)]
pub struct LatencySnapshot {
    /// From the data is logged in the SDK, up until it was added to the store.
    pub e2e: f32,

    /// Delay between the time `RowId` was created and the `ChunkId` was created,
    /// i.e. the time it took to get the data from the `log` call to be batched by the batcher.
    pub log2chunk: f32,

    /// Time between chunk creation and IPC encoding (start of gRPC transmission).
    pub chunk2encode: Option<f32>,

    /// Time between encoding to IPC and decoding again,
    /// e.g. the time it takes to send the data over the network.
    pub transmission: Option<f32>,
}
