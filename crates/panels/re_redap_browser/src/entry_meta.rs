//! Metadata about a server's entries, fetched one part at a time.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use re_async::{AsyncRuntimeHandle, WasmNotSend};
use re_catalog_client::{Asset, AssetCoverage, CatalogClient};
use re_log_types::EntryId;
use re_log_types::external::re_types_core::SegmentId;
use re_mutex::Mutex;
use re_protos::cloud::v1alpha1::ext::AssetMode;
use re_redap_client::{ApiError, ApiErrorKind, ApiResult, ConnectionHandle};
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

pub type SegmentIdsRef = Arc<SegmentIds>;

/// The segment ids of a dataset, in the order of its segment table.
#[derive(Debug, Default)]
pub struct SegmentIds {
    ids: Vec<SegmentId>,

    /// The same ids, to look a segment up without going through all of them.
    set: HashSet<SegmentId>,
}

impl SegmentIds {
    pub fn new(ids: Vec<SegmentId>) -> Self {
        let set = ids.iter().cloned().collect();
        Self { ids, set }
    }

    pub fn ids(&self) -> &[SegmentId] {
        &self.ids
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// How many of `segments` the dataset holds, each segment counted once.
    pub fn count_held(&self, segments: &[SegmentId]) -> usize {
        segments
            .iter()
            .filter(|segment| self.set.contains(*segment))
            .collect::<HashSet<_>>()
            .len()
    }

    /// How many segments of the dataset a coverage listing `segments` under `mode` applies to.
    pub fn count_covered(&self, mode: AssetMode, segments: &[SegmentId]) -> usize {
        let listed = self.count_held(segments);
        match mode {
            AssetMode::OptIn => listed,
            AssetMode::OptOut => self.len().saturating_sub(listed),
        }
    }
}

/// The coverage the server stores for an asset, and the revision a write to it is gated on.
#[derive(Clone, Debug)]
pub struct StoredCoverage {
    pub coverage: AssetCoverage,
    pub revision: u64,

    /// Whether the server stores coverage at all.
    ///
    /// A server without segment properties loads every asset into every segment, and the coverage
    /// it shows cannot be changed.
    pub is_editable: bool,
}

/// The segments of one dataset, as the asset modals name and search them.
#[derive(Clone, Debug)]
pub struct DatasetSegments {
    /// How many segments the dataset holds, once the count has landed.
    pub count: Option<usize>,

    /// The id of each segment, once the list has landed.
    pub ids: ServerValue<SegmentIdsRef, ApiError>,
}

impl Default for DatasetSegments {
    /// The segments of a dataset we don't know: none, and nothing to wait for.
    fn default() -> Self {
        Self {
            count: None,
            ids: ServerValue::Completed(Arc::default()),
        }
    }
}

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

    /// The segment ids from the dataset's segment table, fetched when an asset modal searches
    /// them.
    segment_ids: RequestedObject<SegmentIdsRef, ApiError>,

    /// From the manifest of the dataset's asset dataset.
    assets: RequestedObject<AssetsRef, ApiError>,

    asset_coverage: BTreeMap<SegmentId, RequestedObject<Arc<StoredCoverage>, ApiError>>,
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

    /// The id of each segment the dataset holds, asking for them if nothing has yet.
    pub fn segment_ids(&self, query: EntryMetaQuery<'_>) -> ServerValue<SegmentIdsRef, ApiError> {
        self.0
            .lock()
            .segment_ids
            .request_value(query.runtime, query.egui_ctx, || {
                warn_on_failure(
                    "dataset segment ids",
                    query.dataset_id,
                    segment_ids(query.connection.clone(), query.dataset_id),
                )
            })
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

    /// The segments covered by an asset, fetched on first access.
    pub fn asset_coverage(
        &self,
        query: EntryMetaQuery<'_>,
        asset_id: &SegmentId,
    ) -> ServerValue<Arc<StoredCoverage>, ApiError> {
        self.0
            .lock()
            .asset_coverage
            .entry(asset_id.clone())
            .or_default()
            .request_value(query.runtime, query.egui_ctx, || {
                let connection = query.connection.clone();
                let dataset_id = query.dataset_id;
                let asset_id = asset_id.clone();
                warn_on_failure("asset coverage", dataset_id, async move {
                    stored_coverage(
                        CatalogClient::new(connection)
                            .asset_coverage(dataset_id, asset_id)
                            .await,
                    )
                    .map(Arc::new)
                })
            })
    }

    /// Fetch everything again, keeping what we have until the new values arrive.
    pub fn refresh(&self) {
        let mut guard = self.0.lock();
        let Requests {
            segment_count,
            segment_ids,
            assets,
            asset_coverage,
        } = &mut *guard;
        segment_count.refresh();
        segment_ids.refresh();
        assets.refresh();
        for coverage in asset_coverage.values_mut() {
            coverage.refresh();
        }
    }

    /// Forget the assets we fetched, so the next access fetches them again.
    ///
    /// The rest of the metadata is left alone, since registering an asset doesn't change it.
    pub fn clear_assets(&self) {
        let mut guard = self.0.lock();
        guard.assets.refresh();
        for coverage in guard.asset_coverage.values_mut() {
            coverage.refresh();
        }
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

/// The [`StoredCoverage`] of an asset, from what the server answered to the coverage request.
fn stored_coverage(response: ApiResult<(AssetCoverage, u64)>) -> ApiResult<StoredCoverage> {
    match response {
        Ok((coverage, revision)) => Ok(StoredCoverage {
            coverage,
            revision,
            is_editable: true,
        }),

        // A server without segment properties loads every asset into every
        // segment, which is what the default coverage says.
        Err(err) if err.kind == ApiErrorKind::Unimplemented => {
            re_log::debug_once!("The server does not store asset coverage");
            Ok(StoredCoverage {
                coverage: AssetCoverage::default(),
                revision: 0,
                is_editable: false,
            })
        }

        Err(err) => Err(err),
    }
}

/// How many segments a dataset holds.
///
/// The server has no count endpoint, so this counts the rows of the segment table, one per segment.
async fn segment_count(connection: ConnectionHandle, entry_id: EntryId) -> ApiResult<usize> {
    CatalogClient::new(connection)
        .get_dataset_segment_count(entry_id)
        .await
}

/// The id of each segment a dataset holds, read from its segment table.
async fn segment_ids(connection: ConnectionHandle, entry_id: EntryId) -> ApiResult<SegmentIdsRef> {
    Ok(Arc::new(SegmentIds::new(
        CatalogClient::new(connection)
            .get_dataset_segment_ids(entry_id)
            .await?,
    )))
}

async fn assets(connection: ConnectionHandle, asset_dataset: EntryId) -> ApiResult<AssetsRef> {
    let assets = CatalogClient::new(connection)
        .scan_asset_dataset(asset_dataset)
        .await?;

    Ok(Arc::new(assets))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Coverage counts only segments in the dataset, with repeated selections counted once.
    /// Both modes also support an empty dataset and an empty selection.
    #[test]
    fn coverage_counts_dataset_segments() {
        let ids = SegmentIds::new(vec![
            SegmentId::new("first".into()),
            SegmentId::new("second".into()),
        ]);
        let segments = [
            SegmentId::from("first"),
            SegmentId::from("first"),
            SegmentId::from("missing"),
        ];
        for (mode, selected, empty_selection) in
            [(AssetMode::OptIn, 1, 0), (AssetMode::OptOut, 1, 2)]
        {
            assert_eq!(ids.count_covered(mode, &segments), selected);
            assert_eq!(SegmentIds::default().count_covered(mode, &segments), 0);
            assert_eq!(ids.count_covered(mode, &[]), empty_selection);
        }
    }

    /// A server that doesn't store coverage gives the default coverage, which can't be edited.
    /// Any other failure stays a failure.
    #[test]
    fn a_server_without_segment_properties_covers_every_segment() {
        let origin: re_uri::Origin = "rerun+http://localhost:51234".parse().unwrap();
        let error = |kind| {
            ApiError::with_kind_and_source(
                &origin,
                kind,
                None,
                std::io::Error::other("refused"),
                "asset coverage",
            )
        };

        let coverage = AssetCoverage {
            mode: AssetMode::OptIn,
            segments: vec![SegmentId::from("first")],
        };
        let stored = stored_coverage(Ok((coverage.clone(), 3))).unwrap();
        assert_eq!(stored.coverage, coverage);
        assert_eq!(stored.revision, 3);
        assert!(stored.is_editable);

        let stored = stored_coverage(Err(error(ApiErrorKind::Unimplemented))).unwrap();
        assert_eq!(stored.coverage, AssetCoverage::default());
        assert!(!stored.is_editable);

        let err = stored_coverage(Err(error(ApiErrorKind::PermissionDenied))).unwrap_err();
        assert_eq!(err.kind, ApiErrorKind::PermissionDenied);
    }
}
