//! Standalone view renderer for embedding views in table rows.
//!
//! Renders a view defined by a blueprint independently of the main viewport,
//! by constructing an ad-hoc [`ViewerContext`] with its own recording and blueprint stores.
//! This gives the view the impression of running against a regular recording.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufReader, Cursor};

use ahash::HashMap as AHashMap;
use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{StoreId, StoreKind, TimeReal};

use crate::RERUN_TABLE_BLUEPRINT;
use re_viewer_context::{
    ActiveStoreContext, ApplicationSelectionState, Contents, MissingChunkReporter, StoreCache,
    TimeControl, ViewClass, ViewContextSystemOncePerFrameResult, ViewId, ViewStates,
    ViewSystemIdentifier, ViewerContext, VisitorControlFlow, blueprint_timeline,
};
use re_viewport::execute_systems_for_view;
use re_viewport_blueprint::{ViewBlueprint, ViewportBlueprint};

/// Result of running all once-per-frame context systems for a given recording.
type OncePerFrameResults = IntMap<ViewSystemIdentifier, ViewContextSystemOncePerFrameResult>;

/// Decode an embedded table blueprint from Arrow schema metadata.
///
/// Looks for the `rerun:table_blueprint` key containing base64-encoded `.rbl` data.
/// Returns the decoded [`EntityDb`] blueprint, or `None` if the key is missing
/// or the data cannot be decoded.
pub fn decode_table_blueprint(metadata: &HashMap<String, String>) -> Option<EntityDb> {
    let encoded = metadata.get(RERUN_TABLE_BLUEPRINT)?;
    let bytes = decode_blueprint_value(encoded)?;
    let mut bundle = StoreBundle::from_rrd(
        BufReader::new(Cursor::new(bytes)),
        &re_entity_db::LogSource::EmbeddedTableBlueprint,
    )
    .map_err(|err| {
        re_log::warn_once!("Failed to decode embedded blueprint: {err}");
        err
    })
    .ok()?;

    bundle
        .drain_entity_dbs()
        .find(|db| db.store_kind() == StoreKind::Blueprint)
}

/// Renders views from a blueprint [`EntityDb`], independent of the main viewport.
///
/// Used to embed small view previews (e.g. in table rows) without going through the full
/// viewport layout system. Each call to [`Self::show_preview`] receives a recording to render
/// against, while the blueprint (defining *what* to show) is borrowed from the
/// [`StoreHub`](re_viewer_context::StoreHub).
///
/// If the blueprint contains multiple views, they are rendered left-to-right in the
/// depth-first order defined by the blueprint's container hierarchy.
///
/// A [`RecordingPreviewRenderer`] is constructed fresh at the start of each UI frame; the
/// cached once-per-frame context-system results live for exactly that long, so
/// [`Self::show_preview`] runs those systems at most once per recording per frame even when
/// the same recording is previewed in multiple rows.
pub(crate) struct RecordingPreviewRenderer<'a> {
    /// Blueprint store defining which view(s) to render.
    blueprint: &'a EntityDb,

    /// Blueprint query for resolving the view blueprint.
    blueprint_query: LatestAtQuery,

    /// Ordered list of views to render (left-to-right).
    view_ids: Vec<ViewId>,

    /// Per-frame cache of once-per-frame context-system results, keyed by the recording's
    /// [`StoreId`]. Populated lazily on the first preview of each recording.
    once_per_frame_cache: RefCell<AHashMap<StoreId, OncePerFrameResults>>,
}

impl<'a> RecordingPreviewRenderer<'a> {
    /// Create a [`RecordingPreviewRenderer`] from a pre-decoded blueprint [`EntityDb`].
    ///
    /// Returns `None` if the blueprint does not contain any views.
    /// View ordering follows a depth-first traversal of the blueprint's container tree,
    /// matching the order in which containers list their children.
    pub fn from_blueprint(blueprint: &'a EntityDb) -> Option<Self> {
        let blueprint_query = LatestAtQuery::latest(blueprint_timeline());

        let viewport = ViewportBlueprint::from_db(blueprint, &blueprint_query);

        let mut view_ids: Vec<ViewId> = Vec::new();
        let _ignored = viewport.visit_contents::<()>(&mut |contents, _| {
            if let Contents::View(view_id) = contents {
                view_ids.push(*view_id);
            }
            VisitorControlFlow::Continue
        });

        if view_ids.is_empty() {
            return None;
        }

        Some(Self {
            blueprint,
            blueprint_query,
            view_ids,
            once_per_frame_cache: RefCell::default(),
        })
    }

    /// Number of views this renderer will draw side-by-side.
    pub fn num_views(&self) -> usize {
        self.view_ids.len()
    }

    /// Render the view(s) into the given UI area.
    ///
    /// Creates an ad-hoc [`ViewerContext`] with an isolated recording and store context,
    /// borrowing shared infrastructure (render context, registries, etc.) from `app_ctx`.
    ///
    /// If `recording` is `Some`, the view will render data from that recording.
    /// Otherwise an empty recording is used as a placeholder.
    pub fn show_preview(
        &self,
        app_ctx: &re_viewer_context::AppContext<'_>,
        ui: &mut egui::Ui,
        row_nr: u64,
        recording: Option<&EntityDb>,
        view_states: &mut ViewStates,
    ) {
        if self.view_ids.is_empty() {
            return;
        }

        re_tracing::profile_function!();

        let view_class_registry = app_ctx.view_class_registry;

        // Use the provided recording or fall back to an empty placeholder.
        // We do this so we see at least a view background until the recording is actually loaded.
        let owned_recording;
        let owned_caches;
        let (recording, caches) = if let Some(rec) = recording {
            // Use the store cache from the hub — it's created automatically when recordings are loaded.
            let hub = app_ctx.storage_context;
            let store_cache = hub.hub.store_caches(rec.store_id());
            let Some(caches) = store_cache else {
                // Recording just arrived or hasn't been seen by the hub yet. Try again later.
                ui.request_repaint();
                return;
            };
            (rec, caches)
        } else {
            // We don't have a recording yet
            let recording_store_id = StoreId::new(
                StoreKind::Recording,
                "___preview_renderer___",
                "empty_placeholder",
            );
            owned_recording = EntityDb::new(recording_store_id);
            owned_caches = StoreCache::new(view_class_registry, &owned_recording);

            (&owned_recording, &owned_caches)
        };

        let store_context = ActiveStoreContext {
            blueprint: self.blueprint,
            default_blueprint: None,
            recording,
            caches,
            should_enable_heuristics: false,
        };

        // Derive visualizable/indicated entities from the bootstrapped cache.
        let visualizable_entities_per_visualizer =
            caches.visualizable_entities_for_visualizer_systems();
        let indicated_entities_per_visualizer = caches.indicated_entities_per_visualizer();

        // Build a TimeControl from the embedded blueprint (reads TimePanelBlueprint
        // for timeline, playback speed, etc.) and validate it against the recording.
        let blueprint_ctx = re_viewer_context::AppBlueprintCtx {
            current_blueprint: self.blueprint,
            default_blueprint: None,
            blueprint_query: self.blueprint_query.clone(),
            command_sender: app_ctx.command_sender,
        };
        let mut time_ctrl = TimeControl::from_blueprint(&blueprint_ctx);
        time_ctrl.update_from_blueprint(&blueprint_ctx, Some(recording));

        // TODO(RR-4257): Don't hack mid-point, and actually store some time control.
        // Seed the time cursor at the midpoint of every timeline so the preview
        // shows a representative frame regardless of which timeline gets selected.
        for (name, _timeline) in recording.timelines() {
            if let Some(range) = recording.time_range_for(&name) {
                let mid = TimeReal::from(range.min)
                    + (TimeReal::from(range.max) - TimeReal::from(range.min)) * 0.5;
                time_ctrl.set_time_cursor_ad_hoc(name, mid);
            }
        }
        let active_timeline = time_ctrl.timeline();
        let store_id = recording.store_id();

        // Resolve each view's blueprint + class once. Views that fail to resolve are kept as
        // `None` so they still occupy a column slot (rendered as a placeholder rectangle).
        struct Resolved<'b> {
            view_id: ViewId,
            view_blueprint: ViewBlueprint,
            view_class: &'b dyn ViewClass,
        }
        let resolved: Vec<Option<Resolved<'_>>> = self
            .view_ids
            .iter()
            .map(|view_id| {
                let view_blueprint =
                    ViewBlueprint::try_from_db(*view_id, self.blueprint, &self.blueprint_query)?;
                let view_class =
                    view_class_registry.get_class_or_log_error(view_blueprint.class_identifier());
                Some(Resolved {
                    view_id: *view_id,
                    view_blueprint,
                    view_class,
                })
            })
            .collect();

        // Build the per-view data-result trees against the shared store context so that the
        // `ViewerContext` can expose results for all views at once.
        let mut query_results = AHashMap::default();
        for r in resolved.iter().flatten() {
            let view_state = view_states.get_mut_or_create(store_id, r.view_id, r.view_class);
            let query_range = r.view_blueprint.query_range(
                self.blueprint,
                &self.blueprint_query,
                active_timeline,
                view_class_registry,
                view_state,
            );
            let query_result = r.view_blueprint.contents.build_data_result_tree(
                &store_context,
                active_timeline,
                view_class_registry,
                &self.blueprint_query,
                &query_range,
                &visualizable_entities_per_visualizer,
                &indicated_entities_per_visualizer,
                app_ctx.app_options,
            );
            query_results.insert(r.view_id, query_result);
        }

        // One shared `ViewerContext` for all views of this recording.
        let connected_receivers = Default::default();
        let blueprint_time_ctrl = TimeControl::default();
        let empty_selection_state = ApplicationSelectionState::default(); // We don't support selecting/hovering in previews yet.
        let ctx = ViewerContext {
            app_ctx: re_viewer_context::AppContext {
                active_store_context: Some(&store_context),
                active_time_ctrl: Some(&time_ctrl),
                selection_state: &empty_selection_state,
                ..app_ctx.clone()
            },
            connected_receivers: &connected_receivers,
            store_context: &store_context,
            visualizable_entities_per_visualizer: &visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer: &indicated_entities_per_visualizer,
            query_results: &query_results,
            time_ctrl: &time_ctrl,
            blueprint_time_ctrl: &blueprint_time_ctrl,
            blueprint_query: &self.blueprint_query,
        };

        // Run once-per-frame context systems at most once per recording per frame. The cache
        // is an instance field on `self`, and `self` is constructed freshly each frame, so it
        // is naturally scoped to the current frame.
        let mut once_per_frame_cache = self.once_per_frame_cache.borrow_mut();
        let context_system_once_per_frame_results = once_per_frame_cache
            .entry(store_id.clone())
            .or_insert_with(|| {
                view_class_registry.run_once_per_frame_context_systems(
                    &ctx,
                    resolved
                        .iter()
                        .flatten()
                        .map(|r| r.view_blueprint.class_identifier()),
                )
            });

        // Split the available width equally across the views, left-to-right, with no gap.
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.columns(resolved.len(), |cols| {
            for (col_ui, resolved) in cols.iter_mut().zip(resolved.into_iter()) {
                let Some(Resolved {
                    view_id,
                    view_blueprint,
                    view_class,
                }) = resolved
                else {
                    let rect = col_ui.available_rect_before_wrap();
                    col_ui
                        .painter()
                        .rect_filled(rect, 2.0, col_ui.visuals().extreme_bg_color);
                    col_ui.allocate_rect(rect, egui::Sense::hover());
                    continue;
                };

                let view_state = view_states.get_mut_or_create(store_id, view_id, view_class);
                let (view_query, system_execution_output) = execute_systems_for_view(
                    &ctx,
                    &view_blueprint,
                    view_state,
                    context_system_once_per_frame_results,
                );

                let missing_chunk_reporter =
                    MissingChunkReporter::new(system_execution_output.any_missing_chunks());

                let view_state = view_states.get_mut_or_create(store_id, view_id, view_class);

                // Suppress all inputs while rendering previews: they are meant to be passive.
                let input_before = col_ui.input_mut(|input| {
                    let input_before = input.clone();

                    // Suppress most input.
                    input.raw.modifiers = egui::Modifiers::default();
                    input.raw.events.clear();
                    input.smooth_scroll_delta = egui::Vec2::ZERO;
                    input.focused = false;
                    input.keys_down.clear();
                    input.pointer = egui::PointerState::default();

                    input_before
                });
                let _result = col_ui.push_id((row_nr, view_id), |ui| {
                    ui.disable();
                    view_class.ui(
                        &ctx,
                        &missing_chunk_reporter,
                        ui,
                        view_state,
                        &view_query,
                        system_execution_output,
                    )
                });
                col_ui.input_mut(|input| {
                    *input = input_before;
                });
            }
        });
    }
}

/// Decode a blueprint metadata value.
///
/// Expected format: `base64:<base64-encoded bytes>`.
fn decode_blueprint_value(value: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let encoded = value.strip_prefix("base64:")?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| {
            re_log::warn_once!("Failed to base64-decode embedded blueprint: {err}");
            err
        })
        .ok()
}
