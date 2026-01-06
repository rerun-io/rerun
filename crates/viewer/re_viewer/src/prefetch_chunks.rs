use re_entity_db::EntityDb;
use re_log_channel::LogReceiverSet;
use re_log_types::AbsoluteTimeRange;
use re_viewer_context::TimeControl;

use crate::StartupOptions;

pub fn prefetch_chunks(
    startup_options: &StartupOptions,
    rx_log: &LogReceiverSet,
    recording: &mut EntityDb,
    time_ctrl: &TimeControl,
) -> Option<()> {
    re_tracing::profile_function!();

    let memory_limit = startup_options.memory_limit.max_bytes.unwrap_or(u64::MAX);
    let total_byte_budget = (0.8 * (memory_limit as f64)) as u64; // Don't completely fill it - we want some headroom for caches etc.

    let current_time = time_ctrl.time_i64()?;
    let timeline = *time_ctrl.timeline()?;

    // Load data from slightly before the current time to give some room for latest-at.
    // This is a bit hacky, but works for now.
    let before_margin = match timeline.typ() {
        re_log_types::TimeType::Sequence => 30,
        re_log_types::TimeType::DurationNs | re_log_types::TimeType::TimestampNs => 1_000_000_000,
    };

    let desired_range = AbsoluteTimeRange::new(
        current_time.saturating_sub(before_margin),
        re_chunk::TimeInt::MAX, // Keep loading until the end (if we have the space for it).
    );

    let options = re_entity_db::ChunkPrefetchOptions {
        timeline,
        desired_range,
        total_byte_budget,

        // TODO(RR-3204): what is a reasonable cap here?
        // We don't request more until this much has been received.
        // Small number = low latency, low throughput.
        // High number = high latency, high throughput.
        // Ideally it should depend on the actual channel bandwidth and latency.
        delta_byte_budget: 500_000,
    };

    let data_source = recording.data_source.as_ref()?;
    let rrd_manifest = &mut recording.rrd_manifest_index;

    if !rrd_manifest.has_manifest() {
        return None;
    }

    let mut found_source = false;

    rx_log.for_each(|rx| {
        if rx.source() == data_source {
            found_source = true;

            if !rx.has_waiting_command_receivers() {
                // TODO(RR-3204): we should probably allow 1-2 things in the queue?
                // Either there is noone on the other side,
                // or they are busy processing previous requests.
                // Let's not enqueue more work for them right now (debounce).
                return;
            }

            let rb = rrd_manifest.prefetch_chunks(&options);

            match rb {
                Ok(rb) => {
                    if 0 < rb.num_rows() {
                        re_log::trace!("Asking for {} more chunks", rb.num_rows());
                        rx.send_command(re_log_channel::LoadCommand::LoadChunks(rb));
                    }
                }
                Err(err) => {
                    re_log::debug_once!("prefetch_chunks failed: {err}");
                }
            }
        }
    });

    if !found_source {
        re_log::debug_once!("Failed to find the data source of the recording");
    }

    None
}
