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

    let memory_limit = startup_options.memory_limit.max_bytes.unwrap_or(i64::MAX);
    let current = re_memory::MemoryUse::capture().used().unwrap_or(0);

    let budget_bytes = memory_limit.saturating_sub(current);

    if budget_bytes <= 0 {
        return None;
    }

    let current_time = time_ctrl.time_i64()?;
    let timeline = time_ctrl.timeline()?;
    let buffer_time = match timeline.typ() {
        re_log_types::TimeType::Sequence => 30,
        re_log_types::TimeType::DurationNs | re_log_types::TimeType::TimestampNs => 5_000_000_000,
    };
    let query_range = AbsoluteTimeRange::new(
        current_time.saturating_sub(buffer_time),
        current_time.saturating_add(buffer_time),
        // re_chunk::TimeInt::MAX,
    );
    let data_source = recording.data_source.as_ref()?;
    let rrd_manifest = &mut recording.rrd_manifest_index;

    #[expect(clippy::question_mark)]
    if rrd_manifest.manifest().is_none() {
        return None;
    }

    let mut found_source = false;

    rx_log.for_each(|rx| {
        if rx.source() == data_source {
            found_source = true;

            let rb = if false {
                //TODO: use this code instead
                rrd_manifest.prefetch_chunks(timeline, query_range, budget_bytes as _)
            } else {
                rrd_manifest.time_range_missing_chunks(timeline, query_range)
            };

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
        re_log::debug!("Failed to find the source");
    }

    None
}
