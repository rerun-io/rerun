//! Standalone view renderer for embedding views in table rows.
//!
//! Renders a view defined by a blueprint independently of the main viewport,
//! with [`crate::prepared_views::PreparedViews`].

use egui::NumExt as _;
use std::cell::RefCell;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{AbsoluteTimeRangeF, EntityPath, StoreId, StoreKind};
use re_sdk_types::blueprint::components::ColumnName;
use re_ui::UiExt as _;

use re_viewer_context::{
    ALL_PREVIEWS_MODIFIER, ActivePreview, ActiveStoreContext, Contents, NeedsRepaint, PreviewState,
    StoreCache, SystemCommand, SystemCommandSender as _, TimeControl, TimeControlCommand, ViewId,
    ViewStates, ViewerContext, blueprint_timeline,
};
use re_viewport_blueprint::ViewBlueprint;

use crate::DisplayRecordBatch;
use crate::blueprint::PreviewsConfig;
use crate::datafusion_table_widget::find_row_batch;
use crate::display_record_batch::DisplayColumn;
use crate::prepared_views::{OncePerFrameCache, PreparedViews, ViewInteraction};

const VIEW_SPACING: f32 = 2.0;

/// View widths follow their preferred aspect ratios at a shared height.
/// Missing, non-finite, and non-positive ratios fall back to the golden ratio.
pub struct PreviewLayout {
    preferred_aspect_ratios: Vec<f32>,
}

impl PreviewLayout {
    fn new(ratios: impl IntoIterator<Item = Option<f32>>) -> Self {
        Self {
            preferred_aspect_ratios: ratios.into_iter().map(valid_aspect_ratio).collect(),
        }
    }

    fn aspect_ratio_sum(&self) -> f32 {
        self.preferred_aspect_ratios.iter().sum()
    }

    fn spacing(&self) -> f32 {
        VIEW_SPACING * self.preferred_aspect_ratios.len().saturating_sub(1) as f32
    }

    pub fn width_for_height(&self, height: f32) -> f32 {
        height * self.aspect_ratio_sum() + self.spacing()
    }

    pub fn height_for_width(&self, width: f32) -> f32 {
        (width - self.spacing()).at_least(0.0) / self.aspect_ratio_sum()
    }

    fn view_rects(&self, rect: egui::Rect) -> impl Iterator<Item = egui::Rect> + '_ {
        let height = self.height_for_width(rect.width()).at_most(rect.height());
        let left = rect.center().x - self.width_for_height(height) * 0.5;
        let top = rect.center().y - height * 0.5;
        self.preferred_aspect_ratios
            .iter()
            .scan(left, move |left, ratio| {
                let view_rect = egui::Rect::from_min_size(
                    egui::pos2(*left, top),
                    egui::vec2(height * ratio, height),
                );
                *left = view_rect.right() + VIEW_SPACING;
                Some(view_rect)
            })
    }
}

fn valid_aspect_ratio(ratio: Option<f32>) -> f32 {
    ratio
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .unwrap_or_else(|| 1.0_f32.midpoint(5.0_f32.sqrt()))
}

fn placeholder_store_id() -> StoreId {
    StoreId::new(
        StoreKind::Recording,
        "___preview_renderer___",
        "empty_placeholder",
    )
}

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

    /// Playback speed of every preview recording, as a multiple of real time.
    playback_speed: f32,

    /// Where every preview starts, as a fraction of the recording's time range.
    start_point: f64,

    /// Populated lazily on the first preview of each recording.
    once_per_frame_cache: RefCell<OncePerFrameCache>,
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
            playback_speed: config.playback_speed,
            start_point: config.start_point,
            once_per_frame_cache: RefCell::default(),
        })
    }

    pub fn data_column_index(&self) -> usize {
        self.data_column_index
    }

    /// Size the grid for the widest known recording in this preview column.
    ///
    /// Ratios come from view state, so loading or changing recording data can reflow the grid.
    /// This happens for example when an initially empty preview column loads its first recording.
    pub fn preferred_layout(
        &self,
        ctx: &re_viewer_context::AppContext<'_>,
        view_states: &ViewStates,
    ) -> PreviewLayout {
        let classes = self
            .view_ids
            .iter()
            .map(|view_id| {
                let view =
                    ViewBlueprint::try_from_db(*view_id, self.blueprint, &self.blueprint_query)?;
                Some(view.class(ctx.view_class_registry))
            })
            .collect::<Vec<_>>();
        let layout_for_recording = |store_id: &StoreId| {
            PreviewLayout::new(
                std::iter::zip(&self.view_ids, &classes).map(|(view_id, class)| {
                    let class = class.as_ref()?;
                    if let Some(state) = view_states.get(store_id, *view_id) {
                        class.preferred_tile_aspect_ratio(state)
                    } else {
                        class.preferred_tile_aspect_ratio(class.new_state().as_ref())
                    }
                }),
            )
        };

        view_states
            .preview_state
            .iter()
            .flat_map(|state| state.iter_active_previews())
            .filter(|(store_id, _)| {
                self.view_ids
                    .iter()
                    .any(|id| view_states.get(store_id, *id).is_some())
            })
            .map(|(store_id, _)| layout_for_recording(store_id))
            .max_by(|a, b| a.aspect_ratio_sum().total_cmp(&b.aspect_ratio_sum()))
            .unwrap_or_else(|| layout_for_recording(&placeholder_store_id()))
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

    /// Render the view(s) into the given UI area with [`PreparedViews`].
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

                owned_recording = EntityDb::new(placeholder_store_id());
                owned_caches = StoreCache::new(view_class_registry, &owned_recording);

                (&owned_recording, &owned_caches)
            }
            None => {
                owned_recording = EntityDb::new(placeholder_store_id());
                owned_caches = StoreCache::new(view_class_registry, &owned_recording);

                (&owned_recording, &owned_caches)
            }
        };

        let store_id = recording.store_id();
        let time_ctrl = if let Some(preview) = preview_state.active_preview_mut(store_id) {
            apply_configured_timeline(&mut preview.time_control, self.timeline, recording);
            apply_start_point(preview, self.start_point, recording);
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
        let mut once_per_frame_cache = self.once_per_frame_cache.borrow_mut();
        let prepared = PreparedViews::new(
            app_ctx,
            store_context,
            &self.blueprint_query,
            &self.view_ids,
            view_states,
            &mut once_per_frame_cache,
            ViewInteraction::Passive, // We don't support selecting/hovering in previews yet.
        );

        let mut views_rect = egui::Rect::NOTHING;

        let layout = PreviewLayout::new(self.view_ids.iter().map(|&view_id| {
            let view_class = prepared.view_class(view_id)?;
            let state = view_states.get(store_id, view_id)?;
            view_class.preferred_tile_aspect_ratio(state)
        }));
        let available_rect = ui.available_rect_before_wrap();
        for ((index, &view_id), rect) in std::iter::zip(
            self.view_ids.iter().enumerate(),
            layout.view_rects(available_rect),
        ) {
            let mut col_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(index)
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            // A view missing from the blueprint keeps its column slot
            // and is drawn as a placeholder rectangle.
            let Some(view_class) = prepared.view_class(view_id) else {
                let rect = col_ui.available_rect_before_wrap();
                col_ui
                    .painter()
                    .rect_filled(rect, 2.0, col_ui.visuals().extreme_bg_color);
                col_ui.allocate_rect(rect, egui::Sense::hover());
                continue;
            };

            views_rect = views_rect.union(col_ui.available_rect_before_wrap());

            let preview_id = (&self.column_name, row_nr, view_id);
            prepared.show_view(&mut col_ui, view_id, view_states, preview_id);

            // Rendering can learn a ratio from newly loaded data. Reflow on the next frame.
            if let Some(view_state) = view_states.get(store_id, view_id)
                && valid_aspect_ratio(view_class.preferred_tile_aspect_ratio(view_state))
                    != layout.preferred_aspect_ratios[index]
            {
                col_ui.request_repaint();
            }
        }
        ui.allocate_rect(available_rect, egui::Sense::hover());

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

        preview_playback_ui(
            ui,
            state,
            recording,
            self.playback_speed,
            row_hovered,
            views_rect,
        );
    }
}

/// The elapsed time and the length of the clip, as shown next to the play button.
fn elapsed_label(time_control: &TimeControl, recording: &EntityDb) -> Option<String> {
    let time = time_control.time()?;
    let range = recording.time_range_for(time_control.timeline_name())?;

    let elapsed = (time.as_f64() - range.min.as_f64()).max(0.0).round() as i64;
    let duration = i64::try_from(range.abs_length()).unwrap_or(i64::MAX);

    if time_control
        .timeline()
        .is_some_and(|timeline| timeline.typ() == re_log_types::TimeType::Sequence)
    {
        Some(format!(
            "{} / {}",
            re_format::format_int(elapsed),
            re_format::format_int(duration)
        ))
    } else {
        let format = |ns| {
            re_format::DurationFormatOptions::default()
                .with_only_seconds(false)
                .with_zero_padded_to(duration)
                .with_max_decimals(1)
                .format_nanos(ns)
        };
        Some(format!("{} / {}", format(elapsed), format(duration)))
    }
}

/// Shows the play button and the elapsed time over the bottom of a preview.
fn preview_playback_ui(
    ui: &mut egui::Ui,
    state: &mut PreviewState,
    recording: &EntityDb,
    speed: f32,
    hovered: bool,
    rect: egui::Rect,
) {
    if !rect.is_finite() {
        return;
    }

    let tokens = ui.tokens();

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
        .image_tint_follows_text_color(true)
        .fill(tokens.preview_controls_fill)
        .corner_radius(egui::CornerRadius::same((PLAY_BUTTON_SIZE / 2.0) as u8));
    let button = controls_ui.add_sized(egui::Vec2::splat(PLAY_BUTTON_SIZE), button_widget);

    let button = button.on_hover_ui(|ui| {
        ui.label(action);
        egui::WidgetAtom::new((
            re_ui::IconText::from_modifiers(ui.ctx().os(), ALL_PREVIEWS_MODIFIER),
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
        if ui.input(|input| input.modifiers.matches_exact(ALL_PREVIEWS_MODIFIER)) {
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

/// Move the preview's time to the `start_point` fraction of its time range, once per timeline.
///
/// Waits until the recording's manifest is complete.
fn apply_start_point(preview: &mut ActivePreview, start_point: f64, recording: &EntityDb) {
    let timeline = *preview.time_control.timeline_name();
    if preview.start_point_timeline == Some(timeline) {
        return;
    }

    let manifest = recording.rrd_manifest_index();
    if manifest.has_manifest() && !manifest.is_manifest_complete() {
        return;
    }

    let Some(range) = std::iter::chain(
        preview.time_control.time_range(recording),
        manifest.timeline_range(&timeline),
    )
    .reduce(|a, b| a.union(b)) else {
        return;
    };

    let time = AbsoluteTimeRangeF::from(range).lerp(start_point);
    let _response = preview.time_control.handle_time_commands(
        None::<&ViewerContext<'_>>,
        recording,
        &[TimeControlCommand::SetTime(time)],
    );
    preview.start_point_timeline = Some(timeline);
}

/// Shows a preview timeline when the grid card/row is hovered, or on every preview while cmd is held.
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
    // Do this outside the if to keep showing the timeline when dragged.
    let was_active = ui.read_response(id).is_some_and(|last_response| {
        last_response.hovered() || last_response.dragged() || last_response.clicked()
    });

    // Cards report no hover while anything is dragged, so also count the scrub bar itself.
    if row_hovered || was_active {
        state.scrub_all.mark_card_hovered(ui.ctx());
    }
    let scrub_all_opacity = state.scrub_all.opacity(ui.ctx(), was_active);

    let Some(preview) = state.active_preview_mut(recording.store_id()) else {
        return;
    };

    // `egui::Id` for memory if the user has used cmd/ctrl drag
    // on the timeline.
    //
    // Used to know if we should always show the hint or not.
    let mem_id = egui::Id::unique("preview-has_multi_dragged");

    let all_previews_modifier_held = ui.input(|i| i.modifiers.matches_exact(ALL_PREVIEWS_MODIFIER));
    let shown_on_own = was_active || (views_rect != egui::Rect::NOTHING && row_hovered);
    let opacity = if shown_on_own { 1.0 } else { scrub_all_opacity };

    if 0.0 < opacity {
        let height = egui::lerp(
            TIMELINE_HEIGHT..=TIMELINE_HEIGHT_ACTIVE,
            ui.animate_bool(id, was_active),
        );
        let timeline_rect = views_rect.with_min_y(views_rect.max.y - height);

        let mut painter = ui.painter().clone();
        painter.multiply_opacity(opacity);

        painter.rect_filled(timeline_rect, 0.0, ui.tokens().preview_timeline_track_color);

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
                    re_ui::IconText::from_modifiers(ui.ctx().os(), ALL_PREVIEWS_MODIFIER),
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

                if all_previews_modifier_held {
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
                painter.rect_filled(
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

    #[test]
    fn preview_layout_uses_golden_ratio_for_missing_or_invalid_ratios() {
        for ratio in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f32::NAN),
            Some(f32::INFINITY),
            Some(f32::NEG_INFINITY),
        ] {
            let layout = PreviewLayout::new([ratio]);
            let width = layout.width_for_height(100.0);
            assert!((width - 161.8034).abs() < 0.0001, "{ratio:?}: {width}");
        }
    }

    #[test]
    fn preview_rects_preserve_ratios_at_size_limits() {
        for (ratios, size, expected) in [
            (vec![1.0], [350.0, 350.0], vec![[350.0, 350.0]]),
            (vec![1.0], [350.0, 200.0], vec![[200.0, 200.0]]),
            (vec![0.5], [350.0, 400.0], vec![[200.0, 400.0]]),
            (vec![0.5, 0.5], [350.0, 348.0], vec![[174.0, 348.0]; 2]),
            (
                vec![1.0, 0.5],
                [302.0, 400.0],
                vec![[200.0, 200.0], [100.0, 200.0]],
            ),
            (vec![2.0], [100.0, 400.0], vec![[100.0, 50.0]]),
        ] {
            let layout = PreviewLayout::new(ratios.into_iter().map(Some));
            let bounds = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), size.into());
            let rects = layout.view_rects(bounds).collect::<Vec<_>>();
            assert_eq!(
                rects
                    .iter()
                    .map(|rect| [rect.width(), rect.height()])
                    .collect::<Vec<_>>(),
                expected,
            );
            let combined = rects
                .iter()
                .copied()
                .fold(egui::Rect::NOTHING, egui::Rect::union);
            assert_eq!(combined.center(), bounds.center());
            assert!(bounds.contains_rect(combined));
        }
    }

    /// What the playback tests drive from the outside.
    struct PlaybackTestState {
        view_states: ViewStates,

        /// Index of the preview the pointer is over, as the card layout passes it to the renderer.
        hovered_preview: Option<usize>,

        /// Playback speed from the table blueprint.
        speed: f32,
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
            .with_context_setup(re_ui::apply_style_and_install_loaders)
            .with_size(egui::vec2(400.0 * recordings.len() as f32, 240.0))
            .build_ui_state(
                move |ui, state: &mut PlaybackTestState| {
                    let PlaybackTestState {
                        view_states,
                        hovered_preview,
                        speed,
                    } = state;

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
                                *speed,
                                *hovered_preview == Some(index),
                                rect,
                            );
                        }
                    });
                },
                PlaybackTestState {
                    view_states,
                    hovered_preview,
                    speed: 1.0,
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
            .click_modifiers(ALL_PREVIEWS_MODIFIER);
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

        // Play-all takes over from the button, and the blueprint speed reaches every preview.
        let state = harness.state_mut();
        state.speed = 2.0;
        state
            .view_states
            .preview_state
            .as_mut()
            .expect("the preview is registered")
            .set_playing(true);
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
        assert_eq!(first.view_ids.len(), 1);
        assert_eq!(second.data_column_index(), 5);
        assert_eq!(second.view_ids.len(), 2);
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
