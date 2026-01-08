use arrow::array::RecordBatch;
use re_chunk::Chunk;
use re_entity_db::EntityDb;
use re_log_types::AbsoluteTimeRange;
use re_redap_client::{ApiResult, ConnectionClient};
use re_viewer_context::TimeControl;

use crate::StartupOptions;

pub fn prefetch_chunks(
    egui_ctx: &egui::Context,
    startup_options: &StartupOptions,
    recording: &mut EntityDb,
    time_ctrl: &TimeControl,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
) -> Option<()> {
    re_tracing::profile_function!();

    let redap_uri = recording.redap_uri()?.clone();
    let origin = redap_uri.origin.clone();

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

        // Batch small chunks together.
        max_bytes_per_request: 1_000_000,

        // TODO(RR-3204): what is a reasonable size here?
        // A high value -> better theoretical bandwidth
        delta_byte_budget: 10_000_000,
    };

    let rrd_manifest = &mut recording.rrd_manifest_index;

    if !rrd_manifest.has_manifest() {
        return None;
    }

    // Receive completed promises:
    for chunk in rrd_manifest.resolve_pending_promises() {
        if let Err(err) = recording.add_chunk(&std::sync::Arc::new(chunk)) {
            re_log::warn_once!("add_chunk failed: {err}");
        }
    }

    let rrd_manifest = &mut recording.rrd_manifest_index;

    if rrd_manifest.has_pending_promises() {
        egui_ctx.request_repaint();
    }

    if let Err(err) = rrd_manifest.prefetch_chunks(&options, &|rb| {
        egui_ctx.request_repaint();
        let connection_registry = connection_registry.clone();
        let origin = origin.clone();
        poll_promise::Promise::spawn_async(async move {
            let mut client = connection_registry.client(origin).await.map_err(|err| {
                re_log::warn_once!("Failed to connect to remote: {err}");
            })?;
            match load_chunks(&mut client, &rb).await {
                Ok(chunks) => Ok(chunks),
                Err(err) => {
                    re_log::warn_once!("load_chunks failed: {err}");
                    Err(())
                }
            }
        })
    }) {
        re_log::warn_once!("prefetch_chunks failed: {err}");
    }

    None
}

/// Takes a dataframe that looks like an [`re_log_encoding::RrdManifest`] (has a `chunk_key` column).
async fn load_chunks(client: &mut ConnectionClient, batch: &RecordBatch) -> ApiResult<Vec<Chunk>> {
    use tokio_stream::StreamExt as _;

    if batch.num_rows() == 0 {
        return Ok(vec![]);
    }

    re_log::trace!("Requesting {} chunk(s) from server…", batch.num_rows());

    let chunk_stream = client.fetch_segment_chunks_by_id(batch).await?;
    let mut chunk_stream =
        re_redap_client::fetch_chunks_response_to_chunk_and_segment_id(chunk_stream);
    let mut all_chunks = Vec::new();
    while let Some(chunks) = chunk_stream.next().await {
        for (chunk, _partition_id) in chunks? {
            all_chunks.push(chunk);
        }
    }

    re_log::trace!("Finished downloading {} chunk(s).", batch.num_rows());

    Ok(all_chunks)
}
