use std::collections::BTreeMap;

use emath::History;
use parking_lot::Mutex;
use re_chunk::Chunk;
use re_chunk_store::{ChunkStoreDiff, ChunkStoreDiffKind, ChunkStoreEvent};
use re_sorbet::{TimestampLocation, TimestampMetadata};
use web_time::SystemTime;

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
#[derive(Clone, Debug, Default)]
pub struct LatencyStats {
    /// The latency from [`TimestampLocation::Log`] until this point,
    /// measured in seconds.
    from_log_until: BTreeMap<TimestampLocation, History<f32>>,
}

impl LatencyStats {
    fn on_new_chunk(
        &mut self,
        now_nanos: i64,
        chunk_timestamps: &TimestampMetadata,
        diff: &ChunkStoreDiff,
    ) {
        let mut chunk_timestamps = chunk_timestamps.clone();

        let min_samples = 0; // 0: we stop displaying e2e latency if input stops
        let max_samples = 8 * 1024; // don't waste too much memory on this - we just need enough to get a good average
        let max_age = 1.0; // don't keep too long of a rolling average, or the stats get outdated.

        if diff.kind != ChunkStoreDiffKind::Addition {
            return;
        }

        chunk_timestamps.insert(
            TimestampLocation::Ingest,
            system_time_from_nanos(now_nanos as u64),
        );

        let Some(log_time) = row_id_timestamp(&diff.chunk) else {
            return;
        };
        chunk_timestamps.insert(TimestampLocation::Log, log_time);

        // We use the chunk id for timing, so we need to get the _original_ id:
        let original_chunk_id = diff.split_source.unwrap_or_else(|| diff.chunk.id());
        chunk_timestamps.insert(
            TimestampLocation::ChunkCreation,
            system_time_from_nanos(original_chunk_id.nanos_since_epoch()),
        );

        let now = now_nanos as f64 / 1e9;

        for (&location, &timestamp) in chunk_timestamps.iter() {
            if location == TimestampLocation::Log {
                continue;
            }

            let history = self
                .from_log_until
                .entry(location)
                .or_insert_with(|| History::new(min_samples..max_samples, max_age));

            if let Ok(duration_since_log) = timestamp.duration_since(log_time) {
                history.add(now, duration_since_log.as_secs_f32());
            }
        }
    }

    /// What is the smoothed latency snapshot?
    pub fn snapshot(&mut self) -> LatencySnapshot {
        let mut secs_since_log = BTreeMap::new();

        // make sure the averages is up-to-date:
        let now = nanos_since_epoch() as f64 / 1e9;
        for (location, history) in &mut self.from_log_until {
            history.flush(now);

            if let Some(average) = history.average() {
                secs_since_log.insert(*location, average);
            }
        }

        dbg!(&secs_since_log);

        LatencySnapshot { secs_since_log }
    }
}

fn row_id_timestamp(chunk: &Chunk) -> Option<SystemTime> {
    // TODO: use first, last, or average over all row ids?
    chunk
        .row_ids()
        .next()
        .map(|row_id| system_time_from_nanos(row_id.nanos_since_epoch()))
}

fn nanos_since_epoch() -> i64 {
    if let Ok(duration_since_epoch) = SystemTime::UNIX_EPOCH.elapsed() {
        duration_since_epoch.as_nanos() as i64
    } else {
        re_log::warn_once!("Broken system clock: unable to get current time since epoch.");
        0
    }
}

fn system_time_from_nanos(nanos: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + web_time::Duration::from_nanos(nanos)
}

/// The latest (smoothed) reading of the latency of the ingestion pipeline.
#[derive(Clone, Debug)]
pub struct LatencySnapshot {
    /// Seconds since the initial log call.
    ///
    /// Only valid if the clocks of the viewer and the SDK are in sync.
    pub secs_since_log: BTreeMap<TimestampLocation, f32>,
}

impl LatencySnapshot {
    /// Get the latency from the initial log call to the ingestion, if available.
    pub fn e2e(&self) -> Option<f32> {
        self.secs_since_log.get(&TimestampLocation::LAST).copied()
    }
}
