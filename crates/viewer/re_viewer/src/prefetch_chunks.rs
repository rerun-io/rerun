use arrow::array::RecordBatch;

use re_chunk::{Chunk, TimeInt};
use re_entity_db::EntityDb;
use re_redap_client::{ApiResult, ConnectionClient};
use re_viewer_context::TimeControl;

use crate::StartupOptions;

pub fn prefetch_chunks_for_active_recording(
    egui_ctx: &egui::Context,
    startup_options: &StartupOptions,
    recording: &mut EntityDb,
    time_ctrl: &TimeControl,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    cache_sizes: u64,
) -> Option<()> {
    re_tracing::profile_function!();

    let current_time = time_ctrl.time_i64()?;
    let timeline = *time_ctrl.timeline()?;

    let redap_uri = recording.redap_uri()?.clone();
    let origin = redap_uri.origin.clone();

    let memory_limit = startup_options
        .memory_limit
        .max_bytes
        .unwrap_or(u64::MAX)
        .saturating_sub(cache_sizes);
    let total_byte_budget = (memory_limit as f64 * 0.75 - 1e8).max(0.0) as u64; // Don't completely fill it - we want some headroom for caches etc.

    // Load data from slightly before the current time to give some room for latest-at.
    // This is a bit hacky, but works for now.
    let before_margin = match timeline.typ() {
        re_log_types::TimeType::Sequence => 30,
        re_log_types::TimeType::DurationNs | re_log_types::TimeType::TimestampNs => 1_000_000_000,
    };
    let start_time = TimeInt::new_temporal(current_time.saturating_sub(before_margin));

    if !recording.rrd_manifest_index.has_manifest() {
        return None;
    }

    let options = re_entity_db::ChunkPrefetchOptions {
        timeline,
        start_time,
        total_uncompressed_byte_budget: total_byte_budget,

        // Batch small chunks together.
        max_uncompressed_bytes_per_batch: 1_000_000,

        // TODO(RR-3204): what is a reasonable size here?
        // A high value -> better theoretical bandwidth
        max_uncompressed_bytes_in_transit: 10_000_000,
    };

    let (rrd_manifest, storage_engine) = recording.rrd_manifest_index_mut_and_storage_engine();

    if let Err(err) = rrd_manifest.prefetch_chunks(storage_engine.store(), &options, &|rb| {
        egui_ctx.request_repaint();
        let connection_registry = connection_registry.clone();
        let origin = origin.clone();

        let fut = async move {
            let mut client = connection_registry.client(origin).await.map_err(|err| {
                re_log::warn_once!("Failed to connect to remote: {err}");
            })?;
            load_chunks(&mut client, &rb).await.map_err(|err| {
                re_log::warn_once!("{err}");
            })
        };

        // Annoying poll_promise API difference:
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                poll_promise::Promise::spawn_local(fut)
            } else {
                poll_promise::Promise::spawn_async(fut)
            }
        }
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
