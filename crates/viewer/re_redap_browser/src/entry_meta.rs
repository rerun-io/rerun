//! Metadata about a server's entries, fetched one part at a time.

use std::sync::Arc;

use re_async::{AsyncRuntimeHandle, WasmNotSend};
use re_log_types::EntryId;
use re_mutex::Mutex;
use re_redap_client::{ApiError, ApiResult, Asset, ConnectionHandle};
use re_ui::{RequestedObject, ServerValue};

/// Summary of a catalog entry, as reported by the server hosting it.
///
/// Each part is fetched in the background, see [`DatasetRequests`].
#[derive(Clone, Debug)]
pub struct EntryMeta {
    /// How many segments the entry holds.
    pub segments: ServerValue<usize, ApiError>,
}

/// Where to fetch a dataset's metadata from.
#[derive(Clone, Copy)]
pub struct EntryMetaQuery<'a> {
    pub runtime: &'a AsyncRuntimeHandle,
    pub egui_ctx: &'a egui::Context,
    pub connection: &'a ConnectionHandle,
    pub dataset_id: EntryId,
}

pub type AssetsRef = Arc<Vec<Asset>>;

/// The metadata of one dataset, each part fetched the first time something asks for it.
///
/// This lives behind a shared reference to the dataset, hence the mutex.
#[derive(Default)]
pub struct DatasetRequests(Mutex<Requests>);

/// The requests behind one dataset's metadata, one per round trip.
///
/// The value is cloned out of here on every access, so anything bigger than a few words belongs
/// behind an `Arc`.
#[derive(Default)]
struct Requests {
    /// From the dataset's segment table. Only the count is kept, see [`EntryMeta::segments`].
    segment_count: RequestedObject<usize, ApiError>,

    /// From the manifest of the dataset's asset dataset.
    assets: RequestedObject<AssetsRef, ApiError>,
}

impl DatasetRequests {
    /// Everything we know about the dataset, asking for whatever hasn't been asked for yet.
    pub fn meta(&self, query: EntryMetaQuery<'_>) -> EntryMeta {
        let segments =
            self.0
                .lock()
                .segment_count
                .request_value(query.runtime, query.egui_ctx, || {
                    warn_on_failure(
                        "dataset segment count",
                        query.dataset_id,
                        segment_count(query.connection.clone(), query.dataset_id),
                    )
                });

        EntryMeta { segments }
    }

    /// The assets registered for the dataset, as they stand in its asset dataset.
    pub fn assets(
        &self,
        query: EntryMetaQuery<'_>,
        asset_dataset: Option<EntryId>,
    ) -> ServerValue<AssetsRef, ApiError> {
        let Some(asset_dataset) = asset_dataset else {
            // No asset was ever registered for this dataset.
            return ServerValue::Completed(Arc::default());
        };

        self.0
            .lock()
            .assets
            .request_value(query.runtime, query.egui_ctx, || {
                warn_on_failure(
                    "dataset assets",
                    query.dataset_id,
                    assets(query.connection.clone(), asset_dataset),
                )
            })
    }

    /// Fetch everything again, keeping what we have until the new values arrive.
    pub fn refresh(&self) {
        let mut guard = self.0.lock();
        let Requests {
            segment_count,
            assets,
        } = &mut *guard;
        segment_count.refresh();
        assets.refresh();
    }

    /// Forget the assets we fetched, so the next access fetches them again.
    ///
    /// The rest of the metadata is left alone, since registering an asset doesn't change it.
    pub fn clear_assets(&self) {
        self.0.lock().assets.refresh();
    }

    /// Whether the assets we have are older than what the server has.
    ///
    /// True until the first fetch lands, and again from [`Self::clear_assets`] until the fetch
    /// behind it lands. Nothing is fetched until something asks for the assets, so this also stays
    /// true until then.
    pub fn assets_pending(&self) -> bool {
        matches!(self.0.lock().assets.value(), ServerValue::Pending { .. })
    }
}

/// Logs a failure of `fetch` as well as passing it on, since metadata is only ever decoration.
async fn warn_on_failure<T>(
    what: &'static str,
    entry_id: EntryId,
    fetch: impl Future<Output = ApiResult<T>> + WasmNotSend + 'static,
) -> ApiResult<T> {
    let result = fetch.await;
    if let Err(err) = &result {
        re_log::warn_once!("Failed to fetch {what}: {err}\nEntry: {entry_id}");
    }

    result
}

/// How many segments a dataset holds.
///
/// The server has no count endpoint, so this counts the rows of the segment table, one per segment.
async fn segment_count(connection: ConnectionHandle, entry_id: EntryId) -> ApiResult<usize> {
    let client = connection.client().await?;
    client.get_dataset_segment_count(entry_id).await
}

async fn assets(connection: ConnectionHandle, asset_dataset: EntryId) -> ApiResult<AssetsRef> {
    let assets = connection.scan_asset_dataset(asset_dataset).await?;

    Ok(Arc::new(assets))
}
