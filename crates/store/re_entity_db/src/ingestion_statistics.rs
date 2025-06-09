use parking_lot::Mutex;

use re_chunk::RowId;
use re_chunk_store::{ChunkStoreDiffKind, ChunkStoreEvent, ChunkStoreSubscriber};
use re_log_types::StoreId;

/// Statistics about the latency of incoming data to a store.
pub struct IngestionStatistics {
    store_id: StoreId,
    stats: Mutex<LatencyStats>,
}

impl ChunkStoreSubscriber for IngestionStatistics {
    #[inline]
    fn name(&self) -> String {
        "rerun.testing.store_subscribers.IngestionStatistics".into()
    }

    #[inline]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    #[inline]
    fn on_events(&mut self, events: &[ChunkStoreEvent]) {
        for event in events {
            if event.store_id == self.store_id && event.diff.kind == ChunkStoreDiffKind::Addition {
                for row_id in event.diff.chunk.row_ids() {
                    self.on_new_row_id(row_id);
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

    fn on_new_row_id(&self, row_id: RowId) {
        self.stats.lock().on_new_row_id(row_id);
    }

    /// The latest (smoothed) reading of the latency of the ingestion pipeline.
    pub fn latency_snapshot(&self) -> LatencySnapshot {
        self.stats.lock().snapshot()
    }
}

/// Statistics about the latency of incoming data to a store.
#[derive(Clone, Debug)]
pub struct LatencyStats {
    e2e_latency_sec_history: emath::History<f32>,
}

impl Default for LatencyStats {
    fn default() -> Self {
        let min_samples = 0; // 0: we stop displaying e2e latency if input stops
        let max_samples = 1024; // don't waste too much memory on this - we just need enough to get a good average
        let max_age = 1.0; // don't keep too long of a rolling average, or the stats get outdated.
        Self {
            e2e_latency_sec_history: emath::History::new(min_samples..max_samples, max_age),
        }
    }
}

impl LatencyStats {
    fn on_new_row_id(&mut self, row_id: RowId) {
        if let Some(nanos_since_epoch) = nanos_since_epoch() {
            // This only makes sense if the clocks are very good, i.e. if the recording was on the same machine!
            if let Some(nanos_since_log) = nanos_since_epoch.checked_sub(row_id.nanos_since_epoch())
            {
                let now = nanos_since_epoch as f64 / 1e9;
                let sec_since_log = nanos_since_log as f32 / 1e9;

                self.e2e_latency_sec_history.add(now, sec_since_log);
            }
        }
    }

    /// What is the mean latency between the time data was logged in the SDK and the time it was ingested?
    ///
    /// This is based on the clocks of the viewer and the SDK being in sync,
    /// so if the recording was done on another machine, this is likely very inaccurate.
    pub fn snapshot(&mut self) -> LatencySnapshot {
        if let Some(nanos_since_epoch) = nanos_since_epoch() {
            let now = nanos_since_epoch as f64 / 1e9;
            self.e2e_latency_sec_history.flush(now); // make sure the average is up-to-date.
        }

        LatencySnapshot {
            e2e_latency_sec: self.e2e_latency_sec_history.average(),
        }
    }
}

fn nanos_since_epoch() -> Option<u64> {
    if let Ok(duration_since_epoch) = web_time::SystemTime::UNIX_EPOCH.elapsed() {
        Some(duration_since_epoch.as_nanos() as u64)
    } else {
        None
    }
}

/// The latest (smoothed) reading of the latency of the ingestion pipeline.
#[derive(Clone, Copy, Debug)]
pub struct LatencySnapshot {
    /// From the data is logged in the SDK, up until it was added to the store.
    pub e2e_latency_sec: Option<f32>,
}
