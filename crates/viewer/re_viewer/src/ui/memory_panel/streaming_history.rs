use emath::History;
use re_entity_db::StoreBundle;

use super::chunk_event_stats::ChunkEventStats;

/// Tracks server streaming metrics over time.
///
/// Only updated while the memory panel is open.
pub struct StreamingHistory {
    pub bandwidth_bytes_per_sec: History<f64>,
    pub pending_bytes: History<f64>,
    pub loaded_bytes: History<f64>,
    pub total_manifest_bytes: History<f64>,
    pub chunks_in_flight: History<usize>,
    pub batch_cancellations: History<usize>,
    pub chunks_gc_per_frame: History<u64>,

    /// Previous cumulative GC count, used to compute per-frame delta.
    prev_chunks_gc: u64,
}

impl Default for StreamingHistory {
    fn default() -> Self {
        let max_elems = 32 * 1024;
        let max_secs = f32::INFINITY;
        Self {
            bandwidth_bytes_per_sec: History::new(0..max_elems, max_secs),
            pending_bytes: History::new(0..max_elems, max_secs),
            loaded_bytes: History::new(0..max_elems, max_secs),
            total_manifest_bytes: History::new(0..max_elems, max_secs),
            chunks_in_flight: History::new(0..max_elems, max_secs),
            batch_cancellations: History::new(0..max_elems, max_secs),
            chunks_gc_per_frame: History::new(0..max_elems, max_secs),
            prev_chunks_gc: 0,
        }
    }
}

impl StreamingHistory {
    pub fn capture(&mut self, store_bundle: &StoreBundle) {
        let now = re_memory::util::sec_since_start();

        let mut total_bandwidth = 0.0_f64;
        let mut total_pending = 0_u64;
        let mut total_loaded = 0_u64;
        let mut total_manifest = 0_u64;
        let mut total_chunks_in_flight = 0_usize;
        let mut total_cancellations = 0_usize;
        let mut total_chunks_gc = 0_u64;

        for recording in store_bundle.recordings() {
            if !recording.can_fetch_chunks_from_redap() {
                continue;
            }

            let manifest_index = recording.rrd_manifest_index();
            let chunk_requests = manifest_index.chunk_requests();

            let bw = chunk_requests.bandwidth().unwrap_or(0.0);
            if bw.is_finite() {
                total_bandwidth += bw;
            }
            total_pending += chunk_requests.num_on_wire_bytes_pending();
            total_loaded += recording.byte_size_of_physical_chunks();
            total_manifest += manifest_index.full_uncompressed_size();

            for request in chunk_requests.pending_requests() {
                total_chunks_in_flight += request.row_indices.len();
            }

            total_cancellations += chunk_requests
                .recently_canceled
                .iter()
                .map(|(_time, count)| count)
                .sum::<usize>();

            total_chunks_gc += ChunkEventStats::for_store(recording.store_id()).num_chunks_gc;
        }

        let removed_this_frame = total_chunks_gc.saturating_sub(self.prev_chunks_gc);
        self.prev_chunks_gc = total_chunks_gc;

        self.bandwidth_bytes_per_sec.add(now, total_bandwidth);
        self.pending_bytes.add(now, total_pending as f64);
        self.loaded_bytes.add(now, total_loaded as f64);
        self.total_manifest_bytes.add(now, total_manifest as f64);
        self.chunks_in_flight.add(now, total_chunks_in_flight);
        self.batch_cancellations.add(now, total_cancellations);
        self.chunks_gc_per_frame.add(now, removed_this_frame);
    }
}
