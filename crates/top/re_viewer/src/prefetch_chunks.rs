use ahash::HashMap;
use arrow::array::RecordBatch;

use itertools::chain;
use re_async::AsyncRuntimeHandle;
use re_chunk::Chunk;
use re_entity_db::{ChunkFetcher, ChunkPrefetchOptions, RemainingByteBudget, StoreBundle};
use re_log_types::StoreId;
use re_redap_client::{ApiResult, ConnectionClient};

pub enum RecordingOpenKind {
    Active,
    Preview,
    Inactive,
}

/// Info needed to prefetch chunks for a single recording.
pub struct RecordingPrefetchInfo {
    pub store_id: StoreId,
    pub open_kind: RecordingOpenKind,
    pub time_cursor: Option<re_entity_db::PrefetchTimeCursor>,
    pub origin: re_uri::Origin,
}

/// Prefetch chunks for multiple recordings with a shared memory budget
/// between the recordings. Prioritizes the active recording and preview
/// recordings over background recordings.
pub fn prefetch_chunks_for_recordings(
    egui_ctx: &egui::Context,
    async_runtime: &AsyncRuntimeHandle,
    store_bundle: &mut StoreBundle,
    recordings_info: &HashMap<StoreId, RecordingPrefetchInfo>,
    total_bytes_in_memory: u64,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    options: &ChunkPrefetchOptions,
) {
    re_tracing::profile_function!();

    struct FetchState<'a> {
        store_id: StoreId,
        fetcher: ChunkFetcher<'a>,
        origin: re_uri::Origin,
    }

    let mut recordings_stores_with_info = store_bundle
        .recordings_mut()
        .filter_map(|recording| {
            if !recording.can_fetch_chunks_from_redap() {
                return None;
            }

            let info = recordings_info.get(recording.store_id())?;

            let (rrd_manifest, storage_engine) =
                recording.rrd_manifest_index_mut_and_storage_engine();

            Some((info, rrd_manifest, storage_engine))
        })
        .collect::<Vec<_>>();

    // Compute total in-flight bytes across all recordings upfront,
    // so we know the remaining wire budget before creating any fetchers.
    let total_in_flight_bytes: u64 = recordings_stores_with_info
        .iter()
        .map(|(_, manifest, _)| manifest.chunk_requests().num_on_wire_bytes_pending())
        .sum();
    let mut budget = RemainingByteBudget::new(
        total_bytes_in_memory,
        options
            .max_bytes_on_wire_at_once
            .saturating_sub(total_in_flight_bytes),
    );

    // Early out if the budget is full already.
    // The chunks protected by the last pass stay protected.
    if budget.full() {
        return;
    }

    // Update tracked chunk IDs and build priority lists for all recordings.
    let mut active_states: Vec<FetchState<'_>> = Vec::new();
    let mut preview_states: Vec<FetchState<'_>> = Vec::new();
    let mut background_states: Vec<FetchState<'_>> = Vec::new();
    for (info, manifest, storage_engine) in &mut recordings_stores_with_info {
        if let Some(fetcher) = manifest.prepare_chunk_fetcher(
            storage_engine.store(),
            options,
            info.time_cursor,
            &mut budget,
        ) {
            let fetch_state = FetchState {
                store_id: info.store_id.clone(),
                fetcher,
                origin: info.origin.clone(),
            };

            (match info.open_kind {
                RecordingOpenKind::Active => &mut active_states,
                RecordingOpenKind::Preview => &mut preview_states,
                RecordingOpenKind::Inactive => &mut background_states,
            })
            .push(fetch_state);
        }
    }

    {
        let mut active: Vec<_> = active_states.iter_mut().map(|s| &mut s.fetcher).collect();
        let mut previews: Vec<_> = preview_states.iter_mut().map(|s| &mut s.fetcher).collect();
        let mut background: Vec<_> = background_states
            .iter_mut()
            .map(|s| &mut s.fetcher)
            .collect();
        ChunkFetcher::fetch_for_recordings(
            &mut active,
            &mut previews,
            &mut background,
            &mut budget,
            options.max_fetch_stage,
        );
    }

    // Then finish fetching for all
    let results = chain!(active_states, preview_states, background_states)
        .map(|state| {
            let load_fn = make_load_fn(
                egui_ctx,
                async_runtime,
                connection_registry.connection_handle(state.origin.clone()),
            );

            (state.store_id, state.fetcher.finish(&load_fn))
        })
        .collect::<Vec<_>>();

    drop(recordings_stores_with_info);

    for (store_id, result) in results {
        let Some(recording) = store_bundle.get_mut(&store_id) else {
            continue;
        };

        match result {
            Ok(res) => {
                recording.rrd_manifest_index_mut().handle_fetch_result(res);
            }
            Err(err) => {
                re_log::warn_once!("prefetch_chunks failed: {err}");
            }
        }
    }
}

fn make_load_fn<'a>(
    egui_ctx: &'a egui::Context,
    async_runtime: &'a AsyncRuntimeHandle,
    connection: re_redap_client::ConnectionHandle,
) -> impl Fn(RecordBatch) -> re_entity_db::ChunkPromise + 'a {
    move |rb| {
        egui_ctx.request_repaint();
        let egui_ctx = egui_ctx.clone();
        let connection = connection.clone();

        let (sender, promise) = poll_promise::Promise::new();
        async_runtime.spawn_future(async move {
            let result = async {
                let mut client = connection.client().await.map_err(|err| {
                    re_log::warn_once!("Failed to connect to server: {err}");
                })?;
                load_chunks(&mut client, &rb).await.map_err(|err| {
                    re_log::warn_once!("{err}");
                })
            }
            .await;

            // The promise must be fulfilled before the repaint is requested,
            // or the frame it triggers may poll the promise before the result is in it.
            sender.send(result);
            egui_ctx.request_repaint();
        });

        promise
    }
}

/// Takes a dataframe that looks like an [`re_chunk_index::RrdManifest`] (has a `chunk_key` column).
async fn load_chunks(client: &mut ConnectionClient, batch: &RecordBatch) -> ApiResult<Vec<Chunk>> {
    use tokio_stream::StreamExt as _;

    if batch.num_rows() == 0 {
        return Ok(vec![]);
    }

    re_log::trace!("Requesting {} chunk(s) from server…", batch.num_rows());

    let chunk_stream = client.fetch_segment_chunks_by_id(batch).await?;
    let mut chunk_stream = re_redap_client::fetch_chunks_response_to_chunks(
        chunk_stream,
        re_redap_client::FetchedPayloads::Copied,
        None,
    );
    let mut all_chunks = Vec::new();
    while let Some(chunks) = chunk_stream.next().await {
        all_chunks.extend(chunks?);
    }

    re_log::trace!("Finished downloading {} chunk(s).", batch.num_rows());

    Ok(all_chunks)
}
