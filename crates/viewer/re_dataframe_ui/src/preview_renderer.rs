//! Standalone view renderer for embedding views in table rows.
//!
//! Renders a view defined by a blueprint independently of the main viewport,
//! by constructing an ad-hoc [`ViewerContext`] with its own recording and blueprint stores.
//! This gives the view the impression of running against a regular recording.

use ahash::HashMap as AHashMap;
use nohash_hasher::IntMap;
use std::cell::RefCell;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, StoreId, StoreKind};
use re_sdk_types::blueprint::components::ColumnName;
use re_ui::UiExt as _;

use re_viewer_context::{
    ActiveStoreContext, ApplicationSelectionState, Contents, MissingChunkReporter, StoreCache,
    SystemCommand, SystemCommandSender as _, TimeControl, TimeControlCommand, ViewClass,
    ViewContextSystemOncePerFrameResult, ViewId, ViewStates, ViewSystemIdentifier, ViewerContext,
    blueprint_timeline,
};
use re_viewport::execute_systems_for_view;
use re_viewport_blueprint::ViewBlueprint;

use crate::DisplayRecordBatch;
use crate::blueprint::PreviewsConfig;
use crate::datafusion_table_widget::find_row_batch;
use crate::display_record_batch::DisplayColumn;

/// Result of running all once-per-frame context systems for a given recording.
type OncePerFrameResults = IntMap<ViewSystemIdentifier, ViewContextSystemOncePerFrameResult>;

// Only used to pass between logic.
#[cfg_attr(not(target_arch = "wasm32"), expect(clippy::large_enum_variant))]
pub(crate) enum PreviewRecording<'a> {
    Resolved(&'a EntityDb),
    Unresolved(re_uri::DatasetUri),
}

/// Renders views from a blueprint [`EntityDb`], independent of the main viewport.
///
/// Used to embed small view previews (e.g. in table rows) without going through the full
/// viewport layout system.
/// Each renderer owns one source column and its ordered view selection while borrowing the view
/// definitions from the table blueprint.
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

    /// Source column containing the recording reference rendered by this renderer.
    column_name: ColumnName,

    /// Index of the source column in the table data.
    data_column_index: usize,

    /// Ordered list of views to render (left-to-right).
    view_ids: Vec<ViewId>,

    /// Timeline selected for every preview recording.
    timeline: Option<re_log_types::TimelineName>,

    /// Per-frame cache of once-per-frame context-system results, keyed by the recording's
    /// [`StoreId`]. Populated lazily on the first preview of each recording.
    once_per_frame_cache: RefCell<AHashMap<StoreId, OncePerFrameResults>>,
}

impl<'a> RecordingPreviewRenderer<'a> {
    /// Create a renderer for one preview column.
    pub fn from_previews_config(
        blueprint: &'a EntityDb,
        column_name: ColumnName,
        data_column_index: usize,
        view_paths: &[EntityPath],
        config: &PreviewsConfig,
    ) -> Option<Self> {
        if view_paths.is_empty() {
            re_log::warn_once!("Table preview column has no configured views: {column_name:?}");
            return None;
        }

        let blueprint_query = LatestAtQuery::latest(blueprint_timeline());
        let view_ids = view_paths
            .iter()
            .filter_map(|path| match Contents::try_from(path) {
                Some(Contents::View(view_id)) => Some(view_id),
                Some(Contents::Container(_)) | None => {
                    re_log::warn_once!("Table preview references content that is not a view");
                    None
                }
            })
            .collect::<Vec<_>>();

        if view_ids.is_empty() {
            return None;
        }

        Some(Self {
            blueprint,
            blueprint_query,
            column_name,
            data_column_index,
            view_ids,
            timeline: config.timeline,
            once_per_frame_cache: RefCell::default(),
        })
    }

    pub fn data_column_index(&self) -> usize {
        self.data_column_index
    }

    /// Number of views this renderer will draw side-by-side.
    pub fn num_views(&self) -> usize {
        self.view_ids.len()
    }

    pub fn show_preview_for_row(
        &self,
        app_ctx: &re_viewer_context::AppContext<'_>,
        ui: &mut egui::Ui,
        row_nr: u64,
        row_hovered: bool,
        display_record_batches: &[DisplayRecordBatch],
        view_states: &mut ViewStates,
    ) {
        let recording = {
            let preview_state = view_states.preview_state.get_or_insert_default();
            self.resolve_recording_for_row(
                app_ctx,
                display_record_batches,
                row_nr,
                &mut preview_state.requested_uris,
            )
        };

        self.show_preview(app_ctx, ui, row_nr, row_hovered, recording, view_states);
    }

    fn resolve_recording_for_row<'ctx>(
        &self,
        ctx: &'ctx re_viewer_context::AppContext<'ctx>,
        display_record_batches: &[DisplayRecordBatch],
        row_idx: u64,
        already_requested_uris: &mut ahash::HashSet<re_uri::DatasetUri>,
    ) -> Option<PreviewRecording<'ctx>> {
        let (display_record_batch, batch_index) =
            find_row_batch(display_record_batches, row_idx as usize)?;
        let DisplayColumn::Component(column) =
            display_record_batch.columns().get(self.data_column_index)?
        else {
            return None;
        };
        let uri_str = column.string_value_at(batch_index)?;
        let uri = uri_str
            .parse::<re_uri::DatasetUri>()
            .ok()
            .filter(|uri| uri.segment_id.is_some())?;

        if let Some(recording) = ctx.storage_context.hub.find_recording_by_uri(&uri) {
            ctx.storage_context.hub.mark_preview(recording.store_id());
            return Some(PreviewRecording::Resolved(recording));
        }

        let uri = uri.without_fragment();
        if already_requested_uris.insert(uri.clone()) {
            ctx.command_sender
                .send_system(SystemCommand::LoadDataSource(
                    re_data_source::LogDataSource::RedapDatasetSegment {
                        uri: uri.clone(),
                        open_behavior: re_data_source::RecordingOpenBehavior::Background,
                    },
                ));
        }

        Some(PreviewRecording::Unresolved(uri))
    }

    /// Render the view(s) into the given UI area.
    ///
    /// Creates an ad-hoc [`ViewerContext`] with an isolated recording and store context,
    /// borrowing shared infrastructure (render context, registries, etc.) from `app_ctx`.
    ///
    /// If `recording` is `Some`, the view will render data from that recording.
    /// Otherwise an empty recording is used as a placeholder.
    ///
    /// This also renders a timeline at the bottom of hovered preview columns.
    fn show_preview(
        &self,
        app_ctx: &re_viewer_context::AppContext<'_>,
        ui: &mut egui::Ui,
        row_nr: u64,
        row_hovered: bool,
        recording: Option<PreviewRecording<'_>>,
        view_states: &mut ViewStates,
    ) {
        if self.view_ids.is_empty() {
            return;
        }

        re_tracing::profile_function!();

        let view_class_registry = app_ctx.view_class_registry;
        let preview_state = view_states.preview_state.get_or_insert_default();

        // Use the provided recording or fall back to an empty placeholder.
        // We do this so we see at least a view background until the recording is actually loaded.
        let owned_recording;
        let owned_caches;
        let (recording, caches) = match recording {
            Some(PreviewRecording::Resolved(rec)) => {
                // Use the store cache from the hub — it's created automatically when recordings are loaded.
                let hub = app_ctx.storage_context;
                let store_cache = hub.hub.store_caches(rec.store_id());
                let Some(caches) = store_cache else {
                    // Recording just arrived or hasn't been seen by the hub yet. Try again later.
                    ui.request_repaint();
                    return;
                };

                // Register this recording so the shared preview `TimeControl` knows about it
                // and can advance its loop bounds based on the longest registered clip.
                preview_state.register_recording(rec.store_id(), hub.bundle);

                // Request redraw whenever a new preview is registered to start advancing time for it.
                ui.request_repaint();

                (rec, caches)
            }
            Some(PreviewRecording::Unresolved(uri)) => {
                if let Some(err) = app_ctx.last_loading_error_for_uri(&uri) {
                    ui.centered_and_justified(|ui| {
                        ui.error_label(err);
                    });
                    return;
                }

                // We don't have a recording yet
                let recording_store_id = StoreId::new(
                    StoreKind::Recording,
                    "___preview_renderer___",
                    "empty_placeholder",
                );
                owned_recording = EntityDb::new(recording_store_id);
                owned_caches = StoreCache::new(view_class_registry, &owned_recording);

                (&owned_recording, &owned_caches)
            }
            None => {
                // We don't have a recording yet
                let recording_store_id = StoreId::new(
                    StoreKind::Recording,
                    "___preview_renderer___",
                    "empty_placeholder",
                );
                owned_recording = EntityDb::new(recording_store_id);
                owned_caches = StoreCache::new(view_class_registry, &owned_recording);

                (&owned_recording, &owned_caches)
            }
        };

        // Derive visualizable/indicated entities from the bootstrapped cache.
        let visualizable_entities_per_visualizer =
            caches.visualizable_entities_for_visualizer_systems();
        let indicated_entities_per_visualizer = caches.indicated_entities_per_visualizer();

        let store_id = recording.store_id();
        let time_ctrl =
            if let Some(time_control) = preview_state.recording_time_control_mut(store_id) {
                apply_configured_timeline(time_control, self.timeline, recording);
                time_control.clone()
            } else {
                let mut time_control = TimeControl::preview_time_control();
                apply_configured_timeline(&mut time_control, self.timeline, recording);
                time_control
            };

        let store_context = ActiveStoreContext {
            blueprint: self.blueprint,
            default_blueprint: None,
            recording,
            caches,
            time_ctrl: &time_ctrl,
            should_enable_heuristics: false,
        };

        let active_timeline = time_ctrl.timeline();

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
        let blueprint_time_ctrl = TimeControl::default();
        let empty_selection_state = ApplicationSelectionState::default(); // We don't support selecting/hovering in previews yet.
        let ctx = ViewerContext {
            app_ctx: re_viewer_context::AppContext {
                active_store_context: Some(&store_context),
                selection_state: &empty_selection_state,
                ..app_ctx.clone()
            },
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

        let mut views_rect = egui::Rect::NOTHING;

        // Split the available width equally across the views, left-to-right, with no gap.
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.columns(resolved.len(), |cols| {
            for (col_ui, resolved) in std::iter::zip(cols, resolved) {
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

                let view_rect = col_ui.available_rect_before_wrap();

                views_rect = views_rect.union(view_rect);

                // Suppress all inputs while rendering previews: they are meant to be passive.
                let input_before = col_ui.input_mut(|input| {
                    let input_before = input.clone();

                    // Suppress most input.
                    input.modifiers = egui::Modifiers::default();
                    input.raw.events.clear();
                    input.smooth_scroll_delta = egui::Vec2::ZERO;
                    input.focused = false;
                    input.keys_down.clear();
                    input.pointer = egui::PointerState::default();

                    input_before
                });
                let preview_id = (&self.column_name, row_nr, view_id);
                let _result = col_ui.push_id(preview_id, |ui| {
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

                re_viewport::paint_view_loading_indicator(
                    col_ui,
                    preview_id,
                    view_rect,
                    missing_chunk_reporter.any_missing(),
                    recording,
                );
            }
        });

        preview_timeline(
            app_ctx,
            view_states,
            ui,
            &self.column_name,
            row_nr,
            row_hovered,
            recording,
            &time_ctrl,
            views_rect,
        );
    }
}

fn apply_configured_timeline(
    time_control: &mut TimeControl,
    configured_timeline: Option<re_log_types::TimelineName>,
    recording: &EntityDb,
) {
    let Some(configured_timeline) = configured_timeline else {
        return;
    };
    if time_control.timeline_name() == &configured_timeline {
        return;
    }

    let _response = time_control.handle_time_commands(
        None::<&ViewerContext<'_>>,
        recording,
        &[TimeControlCommand::SetActiveTimeline(configured_timeline)],
    );
}

/// Shows a preview timeline when the grid card/row is hovered.
///
/// This timeline can be interacted with to set the time of the preview.
fn preview_timeline(
    app_ctx: &re_viewer_context::AppContext<'_>,
    view_states: &ViewStates,
    ui: &egui::Ui,
    column_name: &ColumnName,
    row_nr: u64,
    row_hovered: bool,
    recording: &EntityDb,
    time_ctrl: &TimeControl,
    views_rect: egui::Rect,
) {
    let id = ui.id().with(("timeline", column_name, row_nr));

    // Do this outside the if to keep showing the timeline when dragged.
    let was_active = ui.read_response(id).is_some_and(|last_response| {
        last_response.hovered() || last_response.dragged() || last_response.clicked()
    });

    let command_pressed = ui.input(|i| i.modifiers.command);
    if command_pressed || was_active || (views_rect != egui::Rect::NOTHING && row_hovered) {
        let width = egui::lerp(4.0..=10.0, ui.animate_bool(id, was_active));
        let timeline_rect = views_rect.with_min_y(views_rect.max.y - width);

        ui.painter()
            .rect_filled(timeline_rect, 0.0, ui.tokens().preview_timeline_track_color);

        if let Some(time) = time_ctrl.time()
            && let Some(range) = recording.time_range_for(time_ctrl.timeline_name())
        {
            let response = ui.interact(
                timeline_rect.expand2(egui::vec2(0.0, 4.0)),
                id,
                egui::Sense::click_and_drag(),
            );

            // Set time to where we clicked/dragged.
            if (response.clicked() || response.dragged())
                && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
            {
                let p = (pos.x - timeline_rect.min.x) / timeline_rect.width();

                let p = p.clamp(0.0, 1.0);

                let time_offset = p as f64 * range.abs_length() as f64;
                let time = range.min.as_f64() + time_offset;

                app_ctx.command_sender.send_system(
                    re_viewer_context::SystemCommand::TimeControlCommands {
                        store_id: recording.store_id().clone(),
                        time_commands: vec![re_viewer_context::TimeControlCommand::SetTime(
                            re_log_types::TimeReal::from(time),
                        )],
                    },
                );

                if command_pressed && let Some(preview_state) = &view_states.preview_state {
                    for (store_id, active_preview) in preview_state.iter_active_previews() {
                        let Some(db) = app_ctx.store_bundle().get(store_id) else {
                            continue;
                        };

                        let Some(range) =
                            db.time_range_for(active_preview.time_control.timeline_name())
                        else {
                            continue;
                        };

                        let time = range.min.as_f64() + time_offset.min(range.abs_length() as f64);

                        app_ctx.command_sender.send_system(
                            re_viewer_context::SystemCommand::TimeControlCommands {
                                store_id: store_id.clone(),
                                time_commands: vec![
                                    re_viewer_context::TimeControlCommand::SetTime(
                                        re_log_types::TimeReal::from(time),
                                    ),
                                ],
                            },
                        );
                    }
                }
            }

            let p = (time.as_f64() - range.min.as_f64()) / range.abs_length() as f64;
            if p > 0.0 {
                ui.painter().rect_filled(
                    timeline_rect
                        .with_max_x(timeline_rect.min.x + timeline_rect.width() * p as f32),
                    0.0,
                    ui.tokens().preview_timeline_progress_color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_keeps_column_specific_views() {
        let blueprint = EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
        let first_views = [ViewId::random().as_entity_path()];
        let second_views = [
            ViewId::random().as_entity_path(),
            ViewId::random().as_entity_path(),
        ];
        let config = PreviewsConfig::default();

        let first = RecordingPreviewRenderer::from_previews_config(
            &blueprint,
            "first".into(),
            3,
            &first_views,
            &config,
        )
        .unwrap();
        let second = RecordingPreviewRenderer::from_previews_config(
            &blueprint,
            "second".into(),
            5,
            &second_views,
            &config,
        )
        .unwrap();

        assert_eq!(first.data_column_index(), 3);
        assert_eq!(first.num_views(), 1);
        assert_eq!(second.data_column_index(), 5);
        assert_eq!(second.num_views(), 2);
    }

    #[test]
    fn preview_uses_configured_timeline() {
        let recording = EntityDb::new(StoreId::random(StoreKind::Recording, "test"));
        let configured_timeline = re_log_types::TimelineName::try_new("configured").unwrap();
        let mut time_control = TimeControl::preview_time_control();

        apply_configured_timeline(&mut time_control, Some(configured_timeline), &recording);

        assert_eq!(time_control.timeline_name(), &configured_timeline);
    }
}
