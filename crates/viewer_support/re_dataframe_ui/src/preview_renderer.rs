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

use re_view::execute_systems_for_view;
use re_viewer_context::{
    ActiveStoreContext, ApplicationSelectionState, Contents, MissingChunkReporter, NeedsRepaint,
    PreviewState, StoreCache, SystemCommand, SystemCommandSender as _, TimeControl,
    TimeControlCommand, ViewClass, ViewContextSystemOncePerFrameResult, ViewId, ViewStates,
    ViewSystemIdentifier, ViewerContext, blueprint_timeline,
};
use re_viewport_blueprint::ViewBlueprint;

use crate::DisplayRecordBatch;
use crate::blueprint::PreviewsConfig;
use crate::datafusion_table_widget::find_row_batch;
use crate::display_record_batch::DisplayColumn;

/// Result of running all once-per-frame context systems for a given recording.
type OncePerFrameResults = IntMap<ViewSystemIdentifier, ViewContextSystemOncePerFrameResult>;

/// Height of the scrub bar at the bottom of a preview, at rest.
const TIMELINE_HEIGHT: f32 = 4.0;

/// Height of the scrub bar while it is hovered or dragged.
const TIMELINE_HEIGHT_ACTIVE: f32 = 10.0;

/// Room above and below the scrub bar that takes clicks for it.
const TIMELINE_INTERACT_PADDING: f32 = 4.0;

/// How far up from the bottom of a preview the scrub bar takes clicks.
const TIMELINE_INTERACT_HEIGHT: f32 = TIMELINE_HEIGHT_ACTIVE + TIMELINE_INTERACT_PADDING;

/// Diameter of the round play button on a preview.
const PLAY_BUTTON_SIZE: f32 = 34.0;

/// Room between the playback controls and the edges of the preview.
const CONTROLS_MARGIN: f32 = 10.0;

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
/// A [`SegmentPreviewRenderer`] is constructed fresh at the start of each UI frame; the
/// cached once-per-frame context-system results live for exactly that long, so
/// [`Self::show_preview`] runs those systems at most once per recording per frame even when
/// the same recording is previewed in multiple rows.
pub(crate) struct SegmentPreviewRenderer<'a> {
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

impl<'a> SegmentPreviewRenderer<'a> {
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
        let value = column.string_value_at(batch_index)?;

        // A path resolves against the route's server, so the preview loads from the same server
        // the table is read from.
        let uri = match ctx.route.origin() {
            Some(base) => re_uri::DatasetUri::parse_with_base(base, &value),
            None => value.parse::<re_uri::DatasetUri>(),
        }
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
                let is_new = preview_state.active_preview(rec.store_id()).is_none();
                preview_state.register_recording(rec.store_id(), hub.bundle);

                // Request redraw whenever a new preview is registered to start advancing time for it.
                if is_new {
                    ui.request_repaint();
                }

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
        let time_ctrl = if let Some(preview) = preview_state.active_preview_mut(store_id) {
            apply_configured_timeline(&mut preview.time_control, self.timeline, recording);
            preview.time_control.clone()
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
        for r in resolved.iter().flatten() {
            view_states.get_mut_or_create(store_id, r.view_id, r.view_class);
        }
        let query_results = re_viewport_blueprint::query_results_for_views(
            &store_context,
            app_ctx.app_caches,
            resolved.iter().flatten().map(|r| &r.view_blueprint),
            view_class_registry,
            &self.blueprint_query,
            view_states,
            &visualizable_entities_per_visualizer,
            &indicated_entities_per_visualizer,
            app_ctx.app_options,
        );

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

        // Split the available width equally across the views, left-to-right, with a narrow gap.
        ui.spacing_mut().item_spacing.x = 2.0;
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

                re_view::paint_view_loading_indicator(
                    col_ui,
                    preview_id,
                    view_rect,
                    missing_chunk_reporter.any_missing(),
                    recording,
                );
            }
        });

        let timeline_id = ui.make_persistent_id(("timeline", &self.column_name, row_nr));

        let state = view_states.preview_state.get_or_insert_default();
        preview_timeline(
            app_ctx,
            state,
            ui,
            timeline_id,
            row_hovered,
            recording,
            views_rect,
        );

        preview_playback_ui(ui, state, recording, row_hovered, views_rect);
    }
}

/// The elapsed time and the length of the clip, as shown next to the play button.
fn elapsed_label(time_control: &TimeControl, recording: &EntityDb) -> Option<String> {
    let time = time_control.time()?;
    let range = recording.time_range_for(time_control.timeline_name())?;

    let elapsed = (time.as_f64() - range.min.as_f64()).max(0.0);
    let duration = range.abs_length() as f64;

    if time_control
        .timeline()
        .is_some_and(|timeline| timeline.typ() == re_log_types::TimeType::Sequence)
    {
        Some(format!("{elapsed:.0} / {duration:.0}"))
    } else {
        Some(format!("{:.1} / {:.1}s", elapsed / 1e9, duration / 1e9))
    }
}

/// Shows the play button and the elapsed time over the bottom of a preview.
fn preview_playback_ui(
    ui: &mut egui::Ui,
    state: &mut PreviewState,
    recording: &EntityDb,
    hovered: bool,
    rect: egui::Rect,
) {
    if !rect.is_finite() {
        return;
    }

    let tokens = ui.tokens();

    // TODO(RR-4810): Read the speed from `PreviewsConfig` and show the actual speed in the UI.
    let speed = state.speed();

    let all_playing = state.playing();

    let Some(preview) = state.active_preview_mut(recording.store_id()) else {
        return;
    };

    let mut commands = Vec::new();
    let mut play_pause_all = None;

    if preview.time_control.speed() != speed {
        commands.push(TimeControlCommand::SetSpeed(speed));
    }

    let id = ui.make_persistent_id(("preview_playback", recording.store_id()));

    let mut playing = preview.play_override.unwrap_or(all_playing || hovered);

    let label = elapsed_label(&preview.time_control, recording);

    // Sensed before the controls are added, so they take clicks over the preview body.
    let body = ui.interact(
        rect.with_max_y(rect.max.y - TIMELINE_INTERACT_HEIGHT),
        id.with("body"),
        egui::Sense::click(),
    );

    let mut controls_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id)
            .max_rect(egui::Rect::from_min_max(
                egui::pos2(
                    rect.left() + CONTROLS_MARGIN,
                    rect.bottom() - TIMELINE_INTERACT_HEIGHT - PLAY_BUTTON_SIZE,
                ),
                egui::pos2(
                    rect.right() - CONTROLS_MARGIN,
                    rect.bottom() - TIMELINE_INTERACT_HEIGHT,
                ),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    controls_ui.spacing_mut().item_spacing.x = 8.0;

    let (icon, action) = if playing {
        (&re_ui::icons::PAUSE, "Pause preview")
    } else {
        (&re_ui::icons::PLAY, "Play preview")
    };
    let button_widget = controls_ui
        .image_button_widget(icon.as_image(), action)
        .fill(tokens.preview_controls_fill)
        .corner_radius(egui::CornerRadius::same((PLAY_BUTTON_SIZE / 2.0) as u8));
    let button = controls_ui.add_sized(egui::Vec2::splat(PLAY_BUTTON_SIZE), button_widget);

    let button = button.on_hover_ui(|ui| {
        ui.label(action);
        egui::WidgetAtom::new((
            re_ui::IconText::from_modifiers(ui.ctx().os(), egui::Modifiers::COMMAND),
            if playing {
                "+ click to pause all previews"
            } else {
                "+ click to play all previews"
            },
        ))
        .show(ui);
    });

    if body.clicked() || button.clicked() {
        playing = !playing;
        // If holding command, play all previews
        if ui.input(|input| input.modifiers.command) {
            play_pause_all = Some(playing);
        } else {
            preview.play_override = Some(playing);
        }
    } else if !hovered && preview.play_override == Some(all_playing) {
        // Don't keep play override if it's the same as the `play all` state.
        preview.play_override = None;
    }

    let wanted_play_state = if playing {
        re_sdk_types::blueprint::components::PlayState::Playing
    } else {
        re_sdk_types::blueprint::components::PlayState::Paused
    };

    if wanted_play_state != preview.time_control.play_state() {
        commands.push(TimeControlCommand::SetPlayState(wanted_play_state));
    }

    if let Some(label) = label {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(7, 4))
            .corner_radius(6)
            .fill(tokens.preview_controls_fill)
            .show(&mut controls_ui, |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(label)
                            .monospace()
                            .size(11.0)
                            .color(tokens.text_default),
                    )
                    .truncate(),
                );
            });
    }

    if !commands.is_empty() {
        let res = preview.time_control.handle_time_commands(
            None::<&ViewerContext<'_>>,
            recording,
            &commands,
        );

        if res.needs_repaint == NeedsRepaint::Yes {
            ui.request_repaint();
        }
    }

    if let Some(play_pause_all) = play_pause_all {
        state.set_playing(play_pause_all);

        // The clicked preview is under the pointer, so its `play_override` holds the new play
        // state until the pointer leaves.
        if let Some(preview) = state.active_preview_mut(recording.store_id()) {
            preview.play_override = Some(play_pause_all);
        }
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
    state: &mut PreviewState,
    ui: &egui::Ui,
    id: egui::Id,
    row_hovered: bool,
    recording: &EntityDb,
    views_rect: egui::Rect,
) {
    let Some(preview) = state.active_preview_mut(recording.store_id()) else {
        return;
    };

    // Do this outside the if to keep showing the timeline when dragged.
    let was_active = ui.read_response(id).is_some_and(|last_response| {
        last_response.hovered() || last_response.dragged() || last_response.clicked()
    });

    // `egui::Id` for memory if the user has used cmd/ctrl drag
    // on the timeline.
    //
    // Used to know if we should always show the hint or not.
    let mem_id = egui::Id::unique("preview-has_multi_dragged");

    let command_pressed = ui.input(|i| i.modifiers.command);
    if command_pressed || was_active || (views_rect != egui::Rect::NOTHING && row_hovered) {
        let height = egui::lerp(
            TIMELINE_HEIGHT..=TIMELINE_HEIGHT_ACTIVE,
            ui.animate_bool(id, was_active),
        );
        let timeline_rect = views_rect.with_min_y(views_rect.max.y - height);

        ui.painter()
            .rect_filled(timeline_rect, 0.0, ui.tokens().preview_timeline_track_color);

        if let Some(time) = preview.time_control.time()
            && let Some(range) = recording.time_range_for(preview.time_control.timeline_name())
        {
            let response = ui.interact(
                timeline_rect.expand2(egui::vec2(0.0, TIMELINE_INTERACT_PADDING)),
                id,
                egui::Sense::click_and_drag(),
            );

            let show_hint = response.dragged()
                && ui.memory_mut(|mem| !mem.data.get_persisted::<bool>(mem_id).unwrap_or(false));

            let mut tooltip = egui::Tooltip::for_widget(&response);
            tooltip.popup = tooltip
                .popup
                .open(show_hint)
                .anchor(timeline_rect)
                .align(egui::RectAlign::TOP)
                .align_alternatives(&[egui::RectAlign::BOTTOM]);

            tooltip.show(|ui| {
                egui::WidgetAtom::new((
                    "Hold",
                    re_ui::IconText::from_modifiers(ui.ctx().os(), egui::Modifiers::COMMAND),
                    "to scrub all previews",
                ))
                .show(ui);
            });

            // Set time to where we clicked/dragged.
            if (response.clicked() || response.dragged())
                && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
            {
                let p = (pos.x - timeline_rect.min.x) / timeline_rect.width();

                let p = p.clamp(0.0, 1.0);

                let time_offset = p as f64 * range.abs_length() as f64;
                let time = range.min.as_f64() + time_offset;

                let mut needs_repaint = NeedsRepaint::No;

                let res = preview.time_control.handle_time_commands(
                    None::<&ViewerContext<'_>>,
                    recording,
                    &[TimeControlCommand::SetTime(time.into())],
                );

                needs_repaint = needs_repaint.or(res.needs_repaint);

                if command_pressed {
                    // Remember that the user has done the multidrag action to not show the hint anymore.
                    ui.memory_mut(|mem| {
                        mem.data.insert_persisted(mem_id, true);
                    });

                    for (store_id, active_preview) in state.iter_active_previews_mut() {
                        if store_id == recording.store_id() {
                            // We already set the time for the dragged recording.
                            continue;
                        }

                        let Some(db) = app_ctx.store_bundle().get(store_id) else {
                            continue;
                        };

                        let Some(range) =
                            db.time_range_for(active_preview.time_control.timeline_name())
                        else {
                            continue;
                        };

                        let time = range.min.as_f64() + time_offset.min(range.abs_length() as f64);

                        let res = active_preview.time_control.handle_time_commands(
                            None::<&ViewerContext<'_>>,
                            db,
                            &[TimeControlCommand::SetTime(time.into())],
                        );

                        needs_repaint = needs_repaint.or(res.needs_repaint);
                    }
                }

                if needs_repaint == NeedsRepaint::Yes {
                    ui.request_repaint();
                }
            }

            let p = if range.abs_length() == 0 {
                0.0
            } else {
                ((time.as_f64() - range.min.as_f64()) / range.abs_length() as f64).clamp(0.0, 1.0)
            };
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
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    /// What the playback tests drive from the outside.
    struct PlaybackTestState {
        view_states: ViewStates,

        /// Index of the preview the pointer is over, as the card layout passes it to the renderer.
        hovered_preview: Option<usize>,
    }

    /// Shows the playback controls of each recording, side by side.
    fn playback_harness(
        recordings: Vec<EntityDb>,
        hovered_preview: Option<usize>,
    ) -> egui_kittest::Harness<'static, PlaybackTestState> {
        let mut view_states = ViewStates::default();
        let preview_state = view_states.preview_state.get_or_insert_default();
        for recording in &recordings {
            preview_state
                .register_recording(recording.store_id(), &re_entity_db::StoreBundle::default());
        }

        egui_kittest::Harness::builder()
            .with_size(egui::vec2(400.0 * recordings.len() as f32, 240.0))
            .build_ui_state(
                move |ui, state: &mut PlaybackTestState| {
                    let PlaybackTestState {
                        view_states,
                        hovered_preview,
                    } = state;

                    re_ui::apply_style_and_install_loaders(ui.ctx());
                    ui.columns(recordings.len(), |columns| {
                        for (index, (ui, recording)) in
                            std::iter::zip(columns.iter_mut(), &recordings).enumerate()
                        {
                            let Some(preview_state) = &mut view_states.preview_state else {
                                continue;
                            };

                            // The renderer picks the timeline each frame before it draws the
                            // controls.
                            if let Some(preview) =
                                preview_state.active_preview_mut(recording.store_id())
                            {
                                apply_configured_timeline(
                                    &mut preview.time_control,
                                    Some(re_log_types::TimelineName::from_static_str(
                                        TEST_TIMELINE,
                                    )),
                                    recording,
                                );
                            }

                            let rect = ui.available_rect_before_wrap();
                            preview_playback_ui(
                                ui,
                                preview_state,
                                recording,
                                *hovered_preview == Some(index),
                                rect,
                            );
                        }
                    });
                },
                PlaybackTestState {
                    view_states,
                    hovered_preview,
                },
            )
    }

    fn is_playing(
        harness: &egui_kittest::Harness<'_, PlaybackTestState>,
        store_id: &StoreId,
    ) -> bool {
        harness
            .state()
            .view_states
            .preview_state
            .as_ref()
            .and_then(|state| state.is_recording_playing(store_id))
            .expect("the preview is registered")
    }

    /// Whether the play-all button is on.
    fn shared_playing(harness: &egui_kittest::Harness<'_, PlaybackTestState>) -> bool {
        harness
            .state()
            .view_states
            .preview_state
            .as_ref()
            .is_some_and(|state| state.playing())
    }

    /// Clicks the first button with this label, with the play-all modifier held.
    fn modifier_click_first(
        harness: &mut egui_kittest::Harness<'_, PlaybackTestState>,
        label: &str,
    ) {
        harness
            .get_all_by_label(label)
            .next()
            .unwrap_or_else(|| panic!("no button labelled {label:?}"))
            .click_modifiers(egui::Modifiers::COMMAND);
        harness.run();
    }

    /// The timeline [`playback_harness`] selects for each preview.
    const TEST_TIMELINE: &str = "frame";

    /// A preview plays while the pointer is over it, the play button overrides that for one
    /// preview, and the play-all button overrides it for every preview.
    #[test]
    fn preview_plays_on_hover_and_follows_the_shared_controls() {
        let recording = EntityDb::new(StoreId::random(StoreKind::Recording, "test"));
        let store_id = recording.store_id().clone();
        let mut harness = playback_harness(vec![recording], None);

        // With the pointer elsewhere, the preview holds still.
        harness.run();
        assert!(!is_playing(&harness, &store_id));

        // The pointer over the preview starts it.
        harness.state_mut().hovered_preview = Some(0);
        harness.run();
        assert!(is_playing(&harness, &store_id));

        // The button pauses this one preview, although the pointer is still over it.
        harness.get_by_label("Pause preview").click();
        harness.run();
        assert!(!is_playing(&harness, &store_id));

        // It stays paused once the pointer leaves.
        harness.state_mut().hovered_preview = None;
        harness.run();
        assert!(!is_playing(&harness, &store_id));

        // Play-all takes over from the button, and sets the speed of every preview.
        let preview_state = harness
            .state_mut()
            .view_states
            .preview_state
            .as_mut()
            .expect("the preview is registered");
        preview_state.set_playing(true);
        preview_state.set_speed(2.0);
        harness.run();

        assert!(is_playing(&harness, &store_id));
        assert_eq!(
            harness
                .state()
                .view_states
                .preview_state
                .as_ref()
                .and_then(|state| state.active_preview(&store_id))
                .map(|p| p.time_control.speed()),
            Some(2.0)
        );
    }

    /// Clicking the hovered preview's play button with the modifier held plays or pauses every
    /// preview. The previews stay that way on the frames that follow, including after the pointer
    /// leaves.
    #[test]
    fn modifier_click_controls_all_previews() {
        let recordings = vec![
            EntityDb::new(StoreId::random(StoreKind::Recording, "first")),
            EntityDb::new(StoreId::random(StoreKind::Recording, "second")),
        ];
        let store_ids = recordings
            .iter()
            .map(|recording| recording.store_id().clone())
            .collect::<Vec<_>>();

        // Only the hovered first preview plays.
        let mut harness = playback_harness(recordings, Some(0));
        harness.run();
        assert!(is_playing(&harness, &store_ids[0]));
        assert!(!is_playing(&harness, &store_ids[1]));

        // Pausing the first preview with the modifier held pauses both, although the pointer is
        // still over the first one.
        modifier_click_first(&mut harness, "Pause preview");
        assert!(!shared_playing(&harness));
        assert!(store_ids.iter().all(|id| !is_playing(&harness, id)));

        // Playing it with the modifier held plays both.
        modifier_click_first(&mut harness, "Play preview");
        assert!(shared_playing(&harness));
        assert!(store_ids.iter().all(|id| is_playing(&harness, id)));

        // They keep playing once the pointer leaves.
        harness.state_mut().hovered_preview = None;
        harness.run();
        assert!(store_ids.iter().all(|id| is_playing(&harness, id)));
    }

    #[test]
    fn renderer_keeps_column_specific_views() {
        let blueprint = EntityDb::new(StoreId::random(StoreKind::Blueprint, "test"));
        let first_views = [ViewId::random().as_entity_path()];
        let second_views = [
            ViewId::random().as_entity_path(),
            ViewId::random().as_entity_path(),
        ];
        let config = PreviewsConfig::default();

        let first = SegmentPreviewRenderer::from_previews_config(
            &blueprint,
            "first".into(),
            3,
            &first_views,
            &config,
        )
        .unwrap();
        let second = SegmentPreviewRenderer::from_previews_config(
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
