use emath::History;
use parking_lot::Mutex;
use re_chunk_store::{
    ChunkDirectLineageReport, ChunkStoreDiff, ChunkStoreDiffKind, ChunkStoreEvent,
};
use re_sorbet::TimestampMetadata;
use saturating_cast::SaturatingCast as _;

/// Statistics about the latency of incoming data to a store.
#[derive(Default)]
pub struct IngestionStatistics {
    stats: Mutex<LatencyStats>,
}

impl Clone for IngestionStatistics {
    fn clone(&self) -> Self {
        Self {
            stats: Mutex::new(self.stats.lock().clone()),
        }
    }
}

impl IngestionStatistics {
    #[inline]
    pub fn on_events(&self, chunk_timestamps: &TimestampMetadata, events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();
        let now_nanos = nanos_since_epoch();
        let mut stats = self.stats.lock();
        for event in events {
            stats.on_new_chunk(now_nanos, chunk_timestamps, &event.diff);
        }
    }
}

impl IngestionStatistics {
    /// The latest (smoothed) reading of the latency of the ingestion pipeline.
    pub fn latency_snapshot(&self) -> LatencySnapshot {
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

    // All the individual parts:
    /// Delay between the time `RowId` was created and the `ChunkId` was created,
    /// i.e. the time it took to get the data from the `log` call to be batched by the batcher.
    log2chunk: History<f32>,

    /// Time between chunk creation and IPC encoding (start of gRPC transmission).
    chunk2encode: History<f32>,

    /// Time between encoding to IPC and decoding again,
    /// e.g. the time it takes to send the data over the network.
    transmission: History<f32>,

    /// Time from the incoming IPC data being decoded to it being ingested into the store.
    decode2ingest: History<f32>,
}

impl Default for LatencyStats {
    fn default() -> Self {
        let min_samples = 0; // 0: we stop displaying e2e latency if input stops
        let max_samples = 8 * 1024; // don't waste too much memory on this - we just need enough to get a good average
        let max_age = 1.0; // don't keep too long of a rolling average, or the stats get outdated.
        Self {
            e2e: History::new(min_samples..max_samples, max_age),
            log2chunk: History::new(min_samples..max_samples, max_age),
            chunk2encode: History::new(min_samples..max_samples, max_age),
            transmission: History::new(min_samples..max_samples, max_age),
            decode2ingest: History::new(min_samples..max_samples, max_age),
        }
    }
}

impl LatencyStats {
    // TODO: review with someone familiar with the matter.
    fn on_new_chunk(
        &mut self,
        now_nanos: i64,
        chunk_timestamps: &TimestampMetadata,
        diff: &ChunkStoreDiff,
    ) {
        if diff.kind != ChunkStoreDiffKind::Addition {
            return;
        }

        let Self {
            e2e,
            log2chunk,
            chunk2encode,
            transmission,
            decode2ingest,
        } = self;

        let now = now_nanos as f64 / 1e9;

        // We use the chunk id for timing, so we need to get the _original_ id:
        let original_chunk_id = if let Some(ChunkDirectLineageReport::SplitFrom(chunk, _siblings)) =
            diff.direct_lineage.as_ref()
        {
            chunk.id()
        } else {
            diff.chunk_before_processing.id()
        };
        let chunk_creation_nanos = original_chunk_id
            .nanos_since_epoch()
            .saturating_cast::<i64>();

        let TimestampMetadata {
            grpc_encoded_at,
            grpc_decoded_at,
        } = chunk_timestamps;

        let grpc_encoded_at_nanos = grpc_encoded_at.and_then(system_time_to_nanos);
        let grpc_decoded_at_nanos = grpc_decoded_at.and_then(system_time_to_nanos);

        for row_id in diff.chunk_before_processing.row_ids() {
            let row_creation_nanos = row_id.nanos_since_epoch().saturating_cast::<i64>();

            // Total:
            if let Some(e2e_nanos) = now_nanos.checked_sub(row_creation_nanos) {
                e2e.add(now, e2e_nanos as f32 / 1e9);
            }

            // First step: log() call to chunk creation (batcher latency):
            if let Some(log2chunk_nanos) = chunk_creation_nanos.checked_sub(row_creation_nanos) {
                log2chunk.add(now, log2chunk_nanos as f32 / 1e9);
            }
        }

        if let Some(grpc_encoded_at_nanos) = grpc_encoded_at_nanos {
            chunk2encode.add(
                now,
                (grpc_encoded_at_nanos - chunk_creation_nanos) as f32 / 1e9,
            );

            if let Some(grpc_decoded_at_nanos) = grpc_decoded_at_nanos {
                transmission.add(
                    now,
                    (grpc_decoded_at_nanos - grpc_encoded_at_nanos) as f32 / 1e9,
                );
            }
        }

        if let Some(grpc_decoded_at_nanos) = grpc_decoded_at_nanos {
            decode2ingest.add(now, (now_nanos - grpc_decoded_at_nanos) as f32 / 1e9);
        }
    }

    /// What is the mean latency between the time data was logged in the SDK and the time it was ingested?
    ///
    /// This is based on the clocks of the viewer and the SDK being in sync,
    /// so if the recording was done on another machine, this is likely very inaccurate.
    pub fn snapshot(&mut self) -> LatencySnapshot {
        let Self {
            e2e,
            log2chunk,
            chunk2encode,
            transmission,
            decode2ingest,
        } = self;

        {
            // make sure the averages is up-to-date:
            let now_nanos = nanos_since_epoch();
            let now = now_nanos as f64 / 1e9;
            e2e.flush(now);
            log2chunk.flush(now);
            chunk2encode.flush(now);
            transmission.flush(now);
            decode2ingest.flush(now);
        }

        LatencySnapshot {
            e2e: e2e.average(),
            log2chunk: log2chunk.average(),
            chunk2encode: chunk2encode.average(),
            transmission: transmission.average(),
            decode2ingest: decode2ingest.average(),
        }
    }
}

fn nanos_since_epoch() -> i64 {
    if let Ok(duration_since_epoch) = web_time::SystemTime::UNIX_EPOCH.elapsed() {
        duration_since_epoch.as_nanos() as i64
    } else {
        re_log::warn_once!("Broken system clock: unable to get current time since epoch.");
        0
    }
}

fn system_time_to_nanos(system_time: web_time::SystemTime) -> Option<i64> {
    system_time
        .duration_since(web_time::SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos() as i64)
}

/// The latest (smoothed) reading of the latency of the ingestion pipeline.
///
/// All measurements are in seconds, and are only valid if the clocks of the viewer and the SDK are in sync.
///
/// They are `None` if unknown.
#[derive(Clone, Copy, Debug)]
pub struct LatencySnapshot {
    /// From the data is logged in the SDK, up until it was added to the store.
    pub e2e: Option<f32>,

    /// Delay between the time `RowId` was created and the `ChunkId` was created,
    /// i.e. the time it took to get the data from the `log` call to be batched by the batcher.
    pub log2chunk: Option<f32>,

    /// Time between chunk creation and IPC encoding (start of gRPC transmission).
    pub chunk2encode: Option<f32>,

    /// Time between encoding to IPC and decoding again,
    /// e.g. the time it takes to send the data over the network.
    pub transmission: Option<f32>,

    /// Time from the incoming IPC data being decoded to it being ingested into the store.
    pub decode2ingest: Option<f32>,
}
