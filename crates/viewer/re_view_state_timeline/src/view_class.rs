use re_log_types::{
    AbsoluteTimeRange, ComponentPath, EntityPath, TimeCell, TimeInt, TimeReal, TimeType,
    TimelineName, TimestampFormat,
};
use re_sdk_types::blueprint::archetypes::TimeAxis;
use re_sdk_types::blueprint::components::LinkAxis;
use re_time_ruler::{MAX_ZIG_WIDTH, TimeRangesUi};
use re_ui::{Help, IconText, MouseButtonText, UiExt as _, icons, list_item};
use re_viewer_context::{
    DataQueryResult, DataResultInteractionAddress, DragAndDropFeedback, GLOBAL_VIEW_ID,
    IdentifiedViewSystem as _, Item, TimeControlCommand, TimeView, ViewClass, ViewClassExt as _,
    ViewClassLayoutPriority, ViewClassRegistryError, ViewContext, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::data::{StateLaneGroup, StateLanePhase, StateLanesData};

// Layout constants (in screen pixels).
const LANE_BAND_HEIGHT: f32 = 22.0;
const LANE_LABEL_HEIGHT: f32 = 14.0;
const LANE_GAP: f32 = 4.0;

/// Vertical gap between the stacked instance lanes of a multi-instance group.
const SUB_LANE_GAP: f32 = 1.0;

const TIME_AXIS_HEIGHT: f32 = 20.0;
const TOP_MARGIN: f32 = 4.0;

/// Phases narrower than this on screen get folded into a merged region with their
/// narrow neighbors. Wide phases always render with their own color.
const MERGE_PHASE_THRESHOLD_PIXEL: f32 = 4.0;

/// One drawable item along a lane: either a single phase or a merged region.
#[derive(Debug)]
enum RenderItem<'a> {
    /// A phase wide enough to render with its own color and label.
    Single {
        phase: &'a StateLanePhase,
        x_start: f32,
        x_end: f32,

        /// End time of the phase (start of the next phase). `None` for the last phase.
        end_time: Option<i64>,
    },

    /// Two or more consecutive narrow visible phases collapsed into one region.
    Merged {
        x_start: f32,
        x_end: f32,
        start_time: i64,

        /// End time of the last phase in the group, if known.
        end_time: Option<i64>,
        count: usize,
    },
}

impl RenderItem<'_> {
    fn x_range(&self) -> (f32, f32) {
        match self {
            Self::Single { x_start, x_end, .. } | Self::Merged { x_start, x_end, .. } => {
                (*x_start, *x_end)
            }
        }
    }
}

/// View state for pan/zoom.
#[derive(Default, re_byte_size::SizeBytes)]
pub struct StateTimelineViewState {
    /// Pan/zoom window, stored per timeline (in the same representation as the timeline panel).
    pub time_views: std::collections::BTreeMap<TimelineName, TimeView>,
}

impl StateTimelineViewState {
    /// The visible time range to query for `timeline`, derived from its pan/zoom.
    ///
    /// `None` until `timeline` has been auto-fit.
    pub fn visible_time_range(&self, timeline: TimelineName) -> Option<AbsoluteTimeRange> {
        let time_view = self.time_views.get(&timeline)?;
        let min = time_view.min;
        let max = min + TimeReal::from(time_view.time_spanned);
        Some(AbsoluteTimeRange::new(min.floor(), max.ceil()))
    }
}

impl ViewState for StateTimelineViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn heap_size_bytes(&self) -> u64 {
        re_byte_size::SizeBytes::heap_size_bytes(self)
    }
}

#[derive(Default)]
pub struct StateTimelineView;

impl StateTimelineView {
    /// Read the configured time-axis link mode for this view.
    fn time_axis_link(
        &self,
        ctx: &ViewerContext<'_>,
        state: &dyn ViewState,
        view_id: ViewId,
        space_origin: &EntityPath,
    ) -> Result<LinkAxis, ViewSystemExecutionError> {
        let view_ctx = self.view_context(ctx, view_id, state, space_origin);
        let time_axis = ViewProperty::from_archetype::<TimeAxis>(&view_ctx);
        Ok(time_axis
            .component_or_fallback::<LinkAxis>(&view_ctx, TimeAxis::descriptor_link().component)?)
    }
}

impl ViewClass for StateTimelineView {
    fn identifier() -> re_sdk_types::ViewClassIdentifier {
        "StateTimeline".into()
    }

    fn display_name(&self) -> &'static str {
        "State timeline"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &icons::VIEW_STATE_TIMELINE
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<StateTimelineViewState>::default()
    }

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        let egui::InputOptions {
            zoom_modifier,
            horizontal_scroll_modifier,
            ..
        } = egui::InputOptions::default(); // This is OK, since we don't allow the user to change these modifiers.

        Help::new("State timeline view")
            .markdown("Shows state transitions as horizontal colored lanes over time.")
            .control("Move time cursor", icons::RIGHT_MOUSE_CLICK)
            .control(
                "Pan",
                (MouseButtonText(egui::PointerButton::Primary), "+", "drag"),
            )
            .control(
                "Pan",
                IconText::from_modifiers_and(os, horizontal_scroll_modifier, icons::SCROLL),
            )
            .control(
                "Zoom",
                IconText::from_modifiers_and(os, zoom_modifier, icons::SCROLL),
            )
            .control("Reset view", ("double", icons::LEFT_MOUSE_CLICK))
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<crate::StateVisualizer>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        Some(2.5)
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();

        // Show every state change stream in a single view by default.
        if ctx
            .indicated_entities_per_visualizer
            .get(&crate::StateVisualizer::identifier())
            .is_some_and(|entities| entities.iter().any(include_entity))
        {
            ViewSpawnHeuristics::root()
        } else {
            ViewSpawnHeuristics::empty()
        }
    }

    fn selection_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        list_item::list_item_scope(ui, "state_timeline_selection_ui", |ui| {
            let ctx = self.view_context(viewer_ctx, view_id, state, space_origin);
            let time_axis = ViewProperty::from_archetype::<TimeAxis>(&ctx);
            let link = time_axis
                .component_or_fallback::<LinkAxis>(&ctx, TimeAxis::descriptor_link().component)?;

            // Only the link mode is editable per-view. The view range is driven by pan/zoom.
            let query_ctx = time_axis.query_context(&ctx);
            if let Some(field) = ctx
                .viewer_ctx
                .reflection()
                .field_reflection(&TimeAxis::descriptor_link())
            {
                re_view::view_property_component_ui(
                    &query_ctx,
                    ui,
                    &time_axis,
                    field.display_name,
                    field,
                );
            }

            // When linked to global, expose the shared view range (stored on the global view).
            if link == LinkAxis::LinkToGlobal
                && let Some(field) = ctx
                    .viewer_ctx
                    .reflection()
                    .field_reflection(&TimeAxis::descriptor_view_range())
            {
                let global_time_axis = ViewProperty::from_archetype_for_view::<TimeAxis>(
                    ctx.viewer_ctx,
                    GLOBAL_VIEW_ID,
                );
                let global_ctx = ViewContext {
                    viewer_ctx,
                    view_id: GLOBAL_VIEW_ID,
                    view_class_identifier: Self::identifier(),
                    space_origin,
                    view_state: state,
                    query_result: &DataQueryResult::default(),
                };
                let global_query_ctx = global_time_axis.query_context(&global_ctx);
                re_view::view_property_component_ui(
                    &global_query_ctx,
                    ui,
                    &global_time_axis,
                    field.display_name,
                    field,
                );
            }

            Ok::<(), ViewSystemExecutionError>(())
        })
        .inner
    }

    /// Accept drops of components onto the state timeline view. For each dropped component, a new
    /// `StateVisualizer` is added that remaps `StateChange.state` from it.
    fn handle_component_drop(
        &self,
        ctx: &ViewerContext<'_>,
        view_id: ViewId,
        component_paths: &[ComponentPath],
        released: bool,
    ) -> DragAndDropFeedback {
        match re_view::handle_component_drop(
            ctx,
            view_id,
            component_paths,
            released,
            crate::StateVisualizer::identifier(),
            re_sdk_types::archetypes::StateChange::descriptor_state().component,
        ) {
            re_view::ComponentDropResult::Accept => DragAndDropFeedback::Accept,
            re_view::ComponentDropResult::CompatibleButAlreadyVisualized => {
                DragAndDropFeedback::Reject(Some("Already visualized"))
            }
            re_view::ComponentDropResult::Incompatible => {
                DragAndDropFeedback::Reject(Some("Not a state component"))
            }
        }
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<StateTimelineViewState>()?;

        // Collect all lane groups from all visualizers.
        let all_groups: Vec<&StateLaneGroup> = system_output
            .iter_visualizer_data::<StateLanesData>()
            .flat_map(|d| d.groups.iter())
            .collect();

        if all_groups.is_empty() {
            let (rect, _) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        "No state data. Drag a string component from the streams tree into this view or add a new visualizer.",
                    );
                });
            });
            return Ok(());
        }

        // Compute data time range.
        let timeline_range = ctx.recording().time_range_for(&query.timeline);
        let timeline_end: Option<i64> = timeline_range.map(|r| r.max.as_i64());
        let (data_min, data_max) = data_time_range(&all_groups, timeline_end);

        // How is the time (X) axis linked? When linked to global, the pan/zoom window is
        // shared with all other plots (e.g. time series views) via the global blueprint view,
        // rather than kept local to this view.
        let link = self.time_axis_link(ctx, state, query.view_id, query.space_origin)?;
        let global_time_axis = (link == LinkAxis::LinkToGlobal)
            .then(|| ViewProperty::from_archetype_for_view::<TimeAxis>(ctx, GLOBAL_VIEW_ID));

        let data_span = (data_max - data_min).max(1.0);

        // When linked to global, derive the pan/zoom window from the shared blueprint view
        // range. Otherwise auto-fit the first time we render this timeline (stored per-view).
        let mut time_view = if let Some(global_time_axis) = &global_time_axis {
            resolve_linked_time_view(
                global_time_axis,
                timeline_range,
                query.latest_at,
                data_min,
                data_max,
            )
        } else {
            *state.time_views.entry(query.timeline).or_insert_with(|| {
                let min = data_min - data_span * 0.05;
                let max = data_max + data_span * 0.05;
                TimeView {
                    min: TimeReal::from(min),
                    time_spanned: max - min,
                }
            })
        };
        let original_time_view = time_view;

        // Allocate the full available rect.
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        if !ui.is_rect_visible(rect) {
            return Ok(());
        }

        // Layout: ruler at the top, lanes below.
        let time_axis_rect = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.right(), rect.top() + TIME_AXIS_HEIGHT),
        );
        let lanes_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + TIME_AXIS_HEIGHT),
            rect.right_bottom(),
        );

        // Build the time↔screen map. A single contiguous segment matches today's
        // state timeline view behavior (no gap collapsing).
        let data_segment = AbsoluteTimeRange::new(
            TimeInt::saturated_temporal_i64(data_min as i64),
            TimeInt::saturated_temporal_i64(data_max.ceil() as i64),
        );
        let time_ranges_ui = TimeRangesUi::new(
            rect.x_range(),
            time_view,
            std::slice::from_ref(&data_segment),
        );

        // The last phase has no real end; it extends to the (slightly expanded) end of the
        // segment, i.e. right up to the zig-zag "end of timeline" band (same as in the time
        // panel). Using the timeline end itself would leave a bare strip the width of the
        // segment expansion between the last phase and the band.
        let open_end_time: Option<f64> = timeline_end
            .and_then(|_| time_ranges_ui.segments.last())
            .map(|segment| segment.time.max.as_f64());

        let current_time = TimeReal::from(query.latest_at.as_i64() as f64);
        let cursor_x = time_ranges_ui.x_from_time_f32(current_time);

        // Time cursor interaction.
        let cursor_response = cursor_x.filter(|x| rect.x_range().contains(*x)).map(|x| {
            const HALF_WIDTH: f32 = 4.0;
            let interact_rect =
                egui::Rect::from_x_y_ranges((x - HALF_WIDTH)..=(x + HALF_WIDTH), rect.y_range());
            ui.interact(
                interact_rect,
                ui.id().with("state_timeline_cursor"),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeColumn)
        });

        // Background.
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.style().visuals.extreme_bg_color);

        // Draw the time ruler at the top.
        let time_type = ctx
            .time_ctrl
            .timeline()
            .map_or(TimeType::Sequence, |tl| tl.typ());
        let timestamp_format = ctx.app_options().timestamp_format;
        re_time_ruler::paint_time_ranges_and_ticks(
            &time_ranges_ui,
            ui,
            &painter.with_clip_rect(time_axis_rect),
            time_axis_rect.y_range(),
            time_type,
            timestamp_format,
        );

        // Separator between ruler and lanes.
        painter.line_segment(
            [time_axis_rect.left_bottom(), time_axis_rect.right_bottom()],
            egui::Stroke::new(1.0, ui.style().visuals.weak_text_color()),
        );

        // Paint a vertical band of the highlighted state phase, behind the lanes.
        if let Some(highlight) = ctx.time_ctrl.highlighted_range()
            && highlight.timeline == query.timeline
            && highlight.kind == re_viewer_context::TimeRangeHighlightKind::StateTimeline
            && let Some(color) = highlight.color
        {
            let x_start = time_ranges_ui
                .x_from_time_f32(TimeReal::from(highlight.range.min.as_i64() as f64))
                .unwrap_or_else(|| rect.left())
                .max(rect.left());
            let x_end = time_ranges_ui
                .x_from_time_f32(TimeReal::from(highlight.range.max.as_i64() as f64))
                .unwrap_or_else(|| rect.right())
                .min(rect.right());
            if x_end > x_start {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x_start, rect.top()),
                        egui::pos2(x_end, rect.bottom()),
                    ),
                    0.0,
                    color,
                );
            }
        }

        // Lane groups: each one is its own widget, stacked vertically inside a ScrollArea.
        let label_color = ui.style().visuals.text_color();
        let mut hovered_entity: Option<&EntityPath> = None;
        let mut hovered_phase: Option<HoveredPhase> = None;
        let mut group_label_anchors: Vec<(egui::Pos2, &StateLaneGroup)> =
            Vec::with_capacity(all_groups.len());
        ui.scope_builder(egui::UiBuilder::new().max_rect(lanes_rect), |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_source(egui::scroll_area::ScrollSource {
                    scroll_bar: true,
                    drag: egui::scroll_area::DragScroll::Never,
                    mouse_wheel: true,
                })
                .show(ui, |ui: &mut egui::Ui| {
                    ui.add_space(TOP_MARGIN);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for group in &all_groups {
                        let result = show_group(
                            ui,
                            group,
                            &time_ranges_ui,
                            time_type,
                            timestamp_format,
                            open_end_time,
                        );
                        group_label_anchors.push((result.label_pos, *group));
                        if result.is_hovered {
                            hovered_entity = Some(&group.entity_path);
                        }
                        if result.hovered_phase.is_some() {
                            hovered_phase = result.hovered_phase;
                        }
                    }

                    // Mark the limits of the timeline with the same zig-zag bands the time
                    // panel uses. Painted over the lanes: the open-ended last phase of each
                    // lane extends slightly under the band, so its teeth carve into the phase.
                    // Painted inside the scroll area so its scroll bar stays on top.
                    re_time_ruler::paint_time_ranges_gaps(
                        &time_ranges_ui,
                        ui,
                        &painter,
                        rect.y_range(),
                    );

                    // Group labels go on top of the zig-zag bands; their translucent
                    // background plates keep them readable over the teeth.
                    let lanes_painter = painter.with_clip_rect(lanes_rect);
                    for (label_pos, group) in &group_label_anchors {
                        paint_group_label(ui, group, label_color, &lanes_painter, *label_pos);
                    }
                });
        });

        // Dragging the time cursor.
        if let Some(cursor_response) = &cursor_response
            && ui.input(|i| {
                i.pointer.primary_pressed()
                    || i.pointer.primary_down()
                    || i.pointer.primary_released()
            })
            && let Some(pos) = cursor_response.interact_pointer_pos()
            && let Some(time) = time_ranges_ui.time_from_x_f32(pos.x)
        {
            ctx.send_time_commands([
                TimeControlCommand::Pause,
                TimeControlCommand::SetTimeClamped(time),
            ]);
        }

        // Secondary (right) click anywhere in the view jumps the time cursor.
        if response.clicked_by(egui::PointerButton::Secondary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Some(time) = time_ranges_ui.time_from_x_f32(pos.x)
        {
            ctx.send_time_commands([
                TimeControlCommand::Pause,
                TimeControlCommand::SetTimeClamped(time),
            ]);
        }

        // Pan: primary- or middle-click drag, plus two-finger touchpad horizontal scroll.
        // Cmd+scroll is routed to `zoom_delta` by egui, so it won't double-fire here.
        let mut pan_dx = 0.0;
        if response.dragged_by(egui::PointerButton::Primary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            pan_dx += response.drag_delta().x;
            ui.ctx().set_cursor_icon(egui::CursorIcon::AllScroll);
        }
        if response.contains_pointer() {
            pan_dx += ui.input(|i| i.smooth_scroll_delta.x);
        }
        if pan_dx != 0.0
            && let Some(new_view) = time_ranges_ui.pan(-pan_dx)
        {
            time_view = new_view;
        }

        // Ctrl/Cmd + scroll to zoom.
        let zoom_delta = ui.input(|i| i.zoom_delta());
        if zoom_delta != 1.0
            && response.contains_pointer()
            && let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos())
            && let Some(new_view) = time_ranges_ui.zoom_at(pointer_pos.x, zoom_delta)
        {
            time_view = new_view;
        }

        // Double click anywhere in the view to reset zoom.
        // Doesn't reset global time cursor.
        if let Some(global_time_axis) = &global_time_axis {
            // Linked to global: persist pan/zoom to the shared blueprint view range.
            if response.double_clicked() {
                global_time_axis.reset_blueprint_component(ctx, TimeAxis::descriptor_view_range());
                ui.request_repaint();
            } else if time_view != original_time_view {
                save_linked_time_view(ctx, global_time_axis, time_view);
                ui.request_repaint();
            }

            // Keep the query window in sync with what we drew: the visualizer derives its
            // `RangeQuery` from `visible_time_range` (i.e. `time_views`). Without this, a view
            // that rendered independently before being linked would keep querying its stale
            // local window while drawing the global one, dropping transitions outside it.
            // Re-query (repaint) whenever that window changes.
            if state.time_views.insert(query.timeline, time_view) != Some(time_view) {
                ui.request_repaint();
            }
        } else if response.double_clicked() {
            state.time_views.remove(&query.timeline);
            ui.request_repaint();
        } else {
            state.time_views.insert(query.timeline, time_view);
        }

        // Publish the hovered phase so other views can highlight the same range.
        if let Some(phase) = hovered_phase {
            let [r, g, b, _] = phase.color.to_array();
            #[expect(clippy::disallowed_methods)]
            let band_color = egui::Color32::from_rgba_unmultiplied(r, g, b, 30);
            let range = AbsoluteTimeRange::new(
                phase.start_time,
                phase
                    .end_time
                    .map_or(TimeInt::MAX, TimeInt::saturated_temporal_i64),
            );
            ctx.send_time_commands([TimeControlCommand::HighlightRange(
                re_viewer_context::TimeRangeHighlight {
                    range,
                    timeline: query.timeline,
                    kind: re_viewer_context::TimeRangeHighlightKind::StateTimeline,
                    color: Some(band_color),
                },
            )]);
        }

        // Time cursor — uses the same triangle-headed style as the time panel.
        // Painted last so it appears above the lanes.
        if let Some(cursor_x) = cursor_x
            && rect.x_range().contains(cursor_x)
        {
            ui.paint_time_cursor(&painter, cursor_response.as_ref(), cursor_x, rect.y_range());
        }

        // Selection: a hovered lane band selects its entity, anywhere else the view.
        let interacted_item = if let Some(entity_path) = hovered_entity {
            Item::DataResult(DataResultInteractionAddress::from_entity_path(
                query.view_id,
                entity_path.clone(),
            ))
        } else {
            Item::View(query.view_id)
        };
        ctx.handle_select_hover_drag_interactions(&response, interacted_item, false);

        Ok(())
    }
}

/// Walk a lane's phases and produce the list of items to render at the current zoom level,
/// merging consecutive narrow visible phases into [`RenderItem::Merged`] regions.
///
/// Invisible phases break the merge chain so that user-hidden states remain hidden
/// rather than being folded into a visible merged region. A run of narrow phases that
/// contains a single phase is emitted as a [`RenderItem::Single`] (no merge marker).
fn compute_render_items<'a>(
    phases: &'a [StateLanePhase],
    lanes_rect: egui::Rect,
    time_ranges_ui: &TimeRangesUi,
    open_end_time: Option<f64>,
) -> Vec<RenderItem<'a>> {
    struct PendingNarrow<'a> {
        phase: &'a StateLanePhase,
        x_start: f32,
        x_end: f32,
        end_time: Option<i64>,
    }

    /// Accumulator for consecutive narrow visible phases. Tracks only the first
    /// pending phase and the current tail, since `flush` never needs anything
    /// in between — emitting a `Single` (count == 1) or a `Merged` (count >= 2)
    /// uses just the first start and the last end.
    #[derive(Default)]
    struct Pending<'a> {
        first: Option<PendingNarrow<'a>>,
        last_x_end: f32,
        last_end_time: Option<i64>,
        count: usize,
    }

    impl<'a> Pending<'a> {
        fn push(&mut self, p: PendingNarrow<'a>) {
            self.last_x_end = p.x_end;
            self.last_end_time = p.end_time;
            self.count += 1;
            if self.first.is_none() {
                self.first = Some(p);
            }
        }

        fn flush(&mut self, items: &mut Vec<RenderItem<'a>>) {
            let count = std::mem::take(&mut self.count);
            let Some(first) = self.first.take() else {
                return;
            };
            if count == 1 {
                items.push(RenderItem::Single {
                    phase: first.phase,
                    x_start: first.x_start,
                    x_end: first.x_end,
                    end_time: first.end_time,
                });
            } else {
                items.push(RenderItem::Merged {
                    x_start: first.x_start,
                    x_end: self.last_x_end,
                    start_time: first.phase.start_time,
                    end_time: self.last_end_time,
                    count,
                });
            }
        }
    }

    let mut items: Vec<RenderItem<'a>> = Vec::new();
    let mut pending = Pending::default();

    for (i, phase) in phases.iter().enumerate() {
        // Gaps break the merge chain.
        if phase.content.is_none() {
            pending.flush(&mut items);
            continue;
        }

        let is_last = i + 1 == phases.len();
        let next_time: Option<f64> = phases
            .get(i + 1)
            .map(|p| p.start_time as f64)
            .or(open_end_time);
        let Some(x_start) = time_ranges_ui.x_from_time_f32(TimeReal::from(phase.start_time as f64))
        else {
            continue;
        };
        let x_end_unclipped = match next_time {
            Some(t) => time_ranges_ui
                .x_from_time_f32(TimeReal::from(t))
                .unwrap_or_else(|| lanes_rect.right()),
            None => lanes_rect.right(),
        };

        // Off-screen to the right: nothing past this is visible either.
        // The post-loop flush below will handle any remaining pending phases.
        if x_start >= lanes_rect.right() {
            break;
        }
        // Off-screen to the left: skip but keep the merge chain going so the next
        // visible phase can still merge with later ones.
        if x_end_unclipped <= lanes_rect.left() {
            continue;
        }

        let visible_x_start = x_start.max(lanes_rect.left());
        let visible_x_end = x_end_unclipped.min(lanes_rect.right());
        let width = visible_x_end - visible_x_start;
        if width <= 0.0 {
            continue;
        }

        if is_last {
            // The last phase is always its own item (never merged) and open-ended. It
            // extends slightly under the zig-zag "end of timeline" band so the band's
            // teeth carve into it instead of leaving background notches along its edge.
            pending.flush(&mut items);
            items.push(RenderItem::Single {
                phase,
                x_start: visible_x_start,
                x_end: (visible_x_end + MAX_ZIG_WIDTH).min(lanes_rect.right()),
                end_time: None,
            });
        } else if width >= MERGE_PHASE_THRESHOLD_PIXEL {
            pending.flush(&mut items);
            items.push(RenderItem::Single {
                phase,
                x_start: visible_x_start,
                x_end: visible_x_end,
                end_time: next_time.map(|t| t as i64),
            });
        } else {
            pending.push(PendingNarrow {
                phase,
                x_start: visible_x_start,
                x_end: visible_x_end,
                end_time: next_time.map(|t| t as i64),
            });
        }
    }
    pending.flush(&mut items);

    items
}

/// Compute the (min, max) time range across all lane groups.
fn data_time_range(groups: &[&StateLaneGroup], timeline_end: Option<i64>) -> (f64, f64) {
    let mut min = f64::MAX;
    let mut max = f64::MIN;
    for group in groups {
        for phase in group.lanes.iter().flat_map(|lane| &lane.phases) {
            let t = phase.start_time as f64;
            min = min.min(t);
            max = max.max(t);
        }
    }
    if let Some(end) = timeline_end {
        max = max.max(end as f64);
    }
    if min > max {
        (0.0, 1.0)
    } else if (max - min).abs() < f64::EPSILON {
        (min - 0.5, max + 0.5)
    } else {
        (min, max)
    }
}

/// Resolve the pan/zoom window shared via the global blueprint view range.
///
/// The range is read without a fallback (none is registered for it in this crate), so an unset or
/// infinite range resolves to the full timeline range — or the data range when the timeline range
/// is unknown.
fn resolve_linked_time_view(
    global_time_axis: &ViewProperty,
    timeline_range: Option<AbsoluteTimeRange>,
    latest_at: TimeInt,
    data_min: f64,
    data_max: f64,
) -> TimeView {
    let view_range = global_time_axis
        .component_or_empty::<re_sdk_types::blueprint::components::TimeRange>(
            TimeAxis::descriptor_view_range().component,
        )
        .ok()
        .flatten()
        .unwrap_or(re_sdk_types::blueprint::components::TimeRange(
            re_sdk_types::datatypes::TimeRange::EVERYTHING,
        ));

    let cursor = re_sdk_types::datatypes::TimeInt(latest_at.as_i64());
    let min = match view_range.start {
        re_sdk_types::datatypes::TimeRangeBoundary::Infinite => {
            timeline_range.map_or(data_min as i64, |r| r.min.as_i64())
        }
        _ => view_range.start.start_boundary_time(cursor).0,
    };
    let max = match view_range.end {
        re_sdk_types::datatypes::TimeRangeBoundary::Infinite => {
            timeline_range.map_or_else(|| data_max.ceil() as i64, |r| r.max.as_i64())
        }
        _ => view_range.end.end_boundary_time(cursor).0,
    };
    let span = ((max - min) as f64).max(1.0);
    TimeView {
        min: TimeReal::from(min as f64),
        time_spanned: span,
    }
}

/// Persist the pan/zoom window to the shared global blueprint view range.
///
/// Both endpoints are rounded (rather than floored/ceiled) so the width is preserved: asymmetric
/// rounding would inflate the range by up to a unit every frame during a continuous pan, feeding
/// back through [`resolve_linked_time_view`] on the next frame.
fn save_linked_time_view(
    ctx: &ViewerContext<'_>,
    global_time_axis: &ViewProperty,
    time_view: TimeView,
) {
    let start = time_view.min.round();
    let end = (time_view.min + TimeReal::from(time_view.time_spanned)).round();
    let new_range =
        re_sdk_types::blueprint::components::TimeRange(re_sdk_types::datatypes::TimeRange {
            start: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                re_sdk_types::datatypes::TimeInt(start.as_i64()),
            ),
            end: re_sdk_types::datatypes::TimeRangeBoundary::Absolute(
                re_sdk_types::datatypes::TimeInt(end.as_i64()),
            ),
        });
    global_time_axis.save_blueprint_component(ctx, &TimeAxis::descriptor_view_range(), &new_range);
}

/// What the lane-group widgets found under the pointer this frame.
struct HoveredPhase {
    start_time: i64,

    /// `None` for an open-ended last phase.
    end_time: Option<i64>,

    color: egui::Color32,
}

/// What [`show_group`] rendered and found under the pointer.
struct ShowGroupResult {
    /// Where the shared group label goes.
    label_pos: egui::Pos2,

    /// Whether the pointer is over one of the group's bands, including gaps.
    is_hovered: bool,

    /// The render item under the pointer, if any. For a merged region this is the
    /// region's full span.
    hovered_phase: Option<HoveredPhase>,
}

/// Render a lane group as a self-contained widget: label space at the top, then one
/// band per instance lane, stacked vertically.
fn show_group(
    ui: &mut egui::Ui,
    group: &StateLaneGroup,
    time_ranges_ui: &TimeRangesUi,
    time_type: TimeType,
    timestamp_format: TimestampFormat,
    open_end_time: Option<f64>,
) -> ShowGroupResult {
    let num_lanes = group.lanes.len();
    let bands_height =
        num_lanes as f32 * LANE_BAND_HEIGHT + num_lanes.saturating_sub(1) as f32 * SUB_LANE_GAP;
    let (response, painter) = ui.allocate_painter(
        egui::vec2(
            ui.available_width(),
            LANE_LABEL_HEIGHT + bands_height + LANE_GAP,
        ),
        egui::Sense::hover(),
    );
    let rect = response.rect;

    let hover_pos = response.hover_pos();
    let merged_fill_inactive = ui.visuals().widgets.inactive.bg_fill;
    let merged_fill_hovered = ui.visuals().widgets.hovered.bg_fill;
    let merged_text_color = ui.visuals().text_color();

    let mut is_hovered = false;
    let mut hovered_phase = None;

    for (instance, lane) in group.lanes.iter().enumerate() {
        let band_top =
            rect.top() + LANE_LABEL_HEIGHT + instance as f32 * (LANE_BAND_HEIGHT + SUB_LANE_GAP);
        let band_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), band_top),
            egui::pos2(rect.right(), band_top + LANE_BAND_HEIGHT),
        );

        if hover_pos.is_some_and(|pos| band_rect.contains(pos)) {
            is_hovered = true;
        }

        // `compute_render_items` uses the rect's x bounds for clipping phases to the
        // visible time range; y is unused. Passing the group's own rect gives the same
        // bounds as the old whole-area lanes_rect since every group spans the full width.
        let render_items = compute_render_items(&lane.phases, rect, time_ranges_ui, open_end_time);

        for item in &render_items {
            let (x_start, x_end) = item.x_range();
            let item_rect = egui::Rect::from_min_max(
                egui::pos2(x_start, band_rect.top()),
                egui::pos2(x_end, band_rect.bottom()),
            );
            let hovered = hover_pos.is_some_and(|pos| item_rect.contains(pos));

            match item {
                RenderItem::Single {
                    phase, end_time, ..
                } => {
                    paint_single_phase(&painter, item_rect, phase, hovered);
                    if hovered && let Some(content) = &phase.content {
                        hovered_phase = Some(HoveredPhase {
                            start_time: phase.start_time,
                            end_time: *end_time,
                            color: content.color,
                        });
                    }
                }
                RenderItem::Merged {
                    start_time,
                    end_time,
                    count,
                    ..
                } => {
                    let fill = if hovered {
                        merged_fill_hovered
                    } else {
                        merged_fill_inactive
                    };
                    paint_merged_phase(&painter, item_rect, *count, fill, merged_text_color);
                    if hovered {
                        hovered_phase = Some(HoveredPhase {
                            start_time: *start_time,
                            end_time: *end_time,
                            color: fill,
                        });
                    }
                }
            }

            if hovered {
                let instance = (num_lanes > 1).then_some(instance);
                show_item_tooltip(ui, item, instance, time_type, timestamp_format);
            }
        }
    }

    ShowGroupResult {
        label_pos: egui::pos2(rect.left() + 4.0, rect.top()),
        is_hovered,
        hovered_phase,
    }
}

/// Paint a group's label (shared by all its instance lanes) on a translucent
/// background-colored plate, so it stays readable where it overlaps the zig-zag
/// "end of timeline" bands.
/// Over the plain background (the common case) the plate is invisible.
fn paint_group_label(
    ui: &egui::Ui,
    group: &StateLaneGroup,
    label_color: egui::Color32,
    painter: &egui::Painter,
    label_pos: egui::Pos2,
) {
    let label_galley = painter.layout_no_wrap(
        group.label.clone(),
        egui::FontId::proportional(11.0),
        label_color,
    );
    let bg_color = ui.visuals().extreme_bg_color;
    #[expect(clippy::disallowed_methods)] // Data-driven visualization color, not a UI theme color.
    let label_bg_color =
        egui::Color32::from_rgba_unmultiplied(bg_color.r(), bg_color.g(), bg_color.b(), 160);
    painter.rect_filled(
        egui::Rect::from_min_size(label_pos, label_galley.size()).expand2(egui::vec2(3.0, 1.0)),
        2.0,
        label_bg_color,
    );
    painter.galley(label_pos, label_galley, label_color);
}

/// Paint one normal phase: filled band (dimmed when not hovered) + clipped label.
fn paint_single_phase(
    painter: &egui::Painter,
    rect: egui::Rect,
    phase: &StateLanePhase,
    hovered: bool,
) {
    let Some(style) = &phase.content else {
        return;
    };

    #[expect(clippy::disallowed_methods)] // Data-driven visualization color, not a UI theme color.
    let fill = if hovered {
        style.color
    } else {
        let [r, g, b, _] = style.color.to_array();
        egui::Color32::from_rgba_unmultiplied(r, g, b, 200)
    };

    painter.add(egui::epaint::RectShape::new(
        rect,
        0.0,
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Outside,
    ));

    if rect.width() - 6.0 > 10.0 {
        painter.with_clip_rect(rect).text(
            egui::pos2(rect.left() + 4.0, rect.top() + 3.0),
            egui::Align2::LEFT_TOP,
            &style.label,
            egui::FontId::proportional(12.0),
            readable_text_color(style.color),
        );
    }
}

/// Paint a merged region: a flat band in a theme widget color signaling that many
/// narrow phases have been collapsed at the current zoom level. The caller picks the
/// fill from `widgets.inactive`/`widgets.hovered` so the hover state stays
/// token-driven rather than relying on an arbitrary multiplier.
fn paint_merged_phase(
    painter: &egui::Painter,
    rect: egui::Rect,
    count: usize,
    fill: egui::Color32,
    text_color: egui::Color32,
) {
    painter.add(egui::epaint::RectShape::new(
        rect,
        0.0,
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Outside,
    ));

    if rect.width() - 6.0 > 24.0 {
        let label = format!("{count} states");
        painter.with_clip_rect(rect).text(
            egui::pos2(rect.left() + 4.0, rect.top() + 3.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(12.0),
            text_color,
        );
    }
}

fn show_item_tooltip(
    ui: &egui::Ui,
    item: &RenderItem<'_>,
    instance: Option<usize>,
    time_type: TimeType,
    timestamp_format: TimestampFormat,
) {
    egui::Tooltip::always_open(
        ui.ctx().clone(),
        ui.layer_id(),
        egui::Id::new("state_tooltip"),
        egui::PopupAnchor::Pointer,
    )
    .show(|ui| {
        let weak = ui.visuals().weak_text_color();
        let small = egui::FontId::proportional(11.0);
        if let Some(instance) = instance {
            ui.label(
                egui::RichText::new(format!("Instance {instance}"))
                    .font(small.clone())
                    .color(weak),
            );
        }
        match item {
            RenderItem::Single {
                phase, end_time, ..
            } => {
                ui.label(phase.content.as_ref().map_or("", |s| s.label.as_str()));
                ui.add_space(4.0);
                let start = TimeCell::new(time_type, phase.start_time).format(timestamp_format);
                ui.label(
                    egui::RichText::new(format!("Start: {start}"))
                        .font(small.clone())
                        .color(weak),
                );
                if let Some(end) = end_time {
                    let end = TimeCell::new(time_type, *end).format(timestamp_format);
                    ui.label(
                        egui::RichText::new(format!("End: {end}"))
                            .font(small)
                            .color(weak),
                    );
                } else {
                    // No end time → open-ended last phase.
                    ui.label(
                        egui::RichText::new("End: ongoing (no later data)")
                            .font(small)
                            .color(weak),
                    );
                }
            }
            RenderItem::Merged {
                start_time,
                end_time,
                count,
                ..
            } => {
                ui.label(format!("{count} states (zoom in to see details)"));
                ui.add_space(4.0);
                let start = TimeCell::new(time_type, *start_time).format(timestamp_format);
                ui.label(
                    egui::RichText::new(format!("Start: {start}"))
                        .font(small.clone())
                        .color(weak),
                );
                if let Some(end) = end_time {
                    let end = TimeCell::new(time_type, *end).format(timestamp_format);
                    ui.label(
                        egui::RichText::new(format!("End: {end}"))
                            .font(small)
                            .color(weak),
                    );
                }
            }
        }
    });
}

/// Choose white or black text depending on background luminance.
fn readable_text_color(bg: egui::Color32) -> egui::Color32 {
    if bg.intensity() > 0.6 {
        egui::Color32::BLACK
    } else {
        egui::Color32::WHITE
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| StateTimelineView.help(ctx));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct a phase list from `(start_time, drawn)` pairs. `drawn = true` is
    /// a visible state; `drawn = false` is a gap. Color/label are unused by
    /// `compute_render_items`, so we leave them dummy.
    fn lane(phases: &[(i64, bool)]) -> Vec<StateLanePhase> {
        phases
            .iter()
            .map(|&(t, drawn)| StateLanePhase {
                start_time: t,
                content: drawn.then(|| crate::data::StateLanePhaseContent {
                    label: String::new(),
                    color: egui::Color32::TRANSPARENT,
                }),
            })
            .collect()
    }

    /// 100-pixel-wide lane rect; combined with a `TimeView` covering `[0, 100]`
    /// this maps one time unit to one pixel, so phase widths in time equal pixel
    /// widths.
    fn unit_rect() -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 22.0))
    }

    fn ranges_ui(t_min: f64, t_max: f64) -> TimeRangesUi {
        let time_view = TimeView {
            min: TimeReal::from(t_min),
            time_spanned: t_max - t_min,
        };
        let segment = AbsoluteTimeRange::new(
            TimeInt::saturated_temporal_i64(t_min as i64),
            TimeInt::saturated_temporal_i64(t_max.ceil() as i64),
        );
        TimeRangesUi::new(
            unit_rect().x_range(),
            time_view,
            std::slice::from_ref(&segment),
        )
    }

    fn is_single(item: &RenderItem<'_>, expected_start: i64) -> bool {
        matches!(item, RenderItem::Single { phase, .. } if phase.start_time == expected_start)
    }

    fn is_open_single(item: &RenderItem<'_>, expected_start: i64) -> bool {
        matches!(
            item,
            RenderItem::Single { phase, end_time: None, .. }
                if phase.start_time == expected_start
        )
    }

    fn is_merged(item: &RenderItem<'_>, expected_start: i64, expected_count: usize) -> bool {
        matches!(
            item,
            RenderItem::Merged { start_time, count, .. }
                if *start_time == expected_start && *count == expected_count
        )
    }

    #[test]
    fn empty_lane_produces_no_items() {
        let lane = lane(&[]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert!(items.is_empty(), "{items:?}");
    }

    #[test]
    fn single_wide_phase_renders_as_single() {
        // One phase covering x=0..100 — well above the merge threshold.
        let lane = lane(&[(0, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
    }

    #[test]
    fn lone_narrow_phase_renders_as_single_not_merged() {
        // Phase 0: x=0..2 (narrow). Phase 1: x=2..100 (wide).
        // The narrow phase has no narrow neighbor to merge with, so it stays Single.
        let lane = lane(&[(0, true), (2, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 2), "{items:?}");
    }

    #[test]
    fn two_consecutive_narrow_phases_merge() {
        // Two narrow (x=0..2, 2..4) + one wide (x=4..100).
        let lane = lane(&[(0, true), (2, true), (4, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_merged(&items[0], 0, 2), "{items:?}");
        assert!(is_single(&items[1], 4), "{items:?}");
    }

    #[test]
    fn wide_phase_breaks_merge_chain() {
        // Wide (0..10), narrow (10..12), wide (12..100) — the lone narrow stays Single.
        let lane = lane(&[(0, true), (10, true), (12, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 3, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 10), "{items:?}");
        assert!(is_single(&items[2], 12), "{items:?}");
    }

    #[test]
    fn invisible_phase_breaks_merge_chain() {
        // narrow visible (0..2), narrow invisible (2..4), narrow visible (4..6), wide (6..100).
        // The two visible narrow phases must NOT merge across the invisible gap.
        let lane = lane(&[(0, true), (2, false), (4, true), (6, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 3, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 4), "{items:?}");
        assert!(is_single(&items[2], 6), "{items:?}");
    }

    #[test]
    fn gap_phase_is_not_drawn_and_bounds_previous_state() {
        // wide state (0..50), gap at 50, wide state (60..100).
        // The gap should not produce a render item, but the first state must end at
        // t=50 (not t=60). The gap also breaks any merge chain.
        let lane = lane(&[(0, true), (50, false), (60, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 2, "{items:?}");
        match &items[0] {
            RenderItem::Single { end_time, .. } => assert_eq!(*end_time, Some(50)),
            item @ RenderItem::Merged { .. } => {
                panic!("expected first item to be Single, got {item:?}")
            }
        }
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 60), "{items:?}");
    }

    #[test]
    fn trailing_gap_truncates_last_state() {
        // wide state (0..70), gap at 70 — the lane ends with no active state.
        // The state's end_time must be the gap's start, and the gap itself produces no item.
        let lane = lane(&[(0, true), (70, false)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 1, "{items:?}");
        match &items[0] {
            RenderItem::Single { end_time, .. } => assert_eq!(*end_time, Some(70)),
            item @ RenderItem::Merged { .. } => panic!("expected Single, got {item:?}"),
        }
    }

    #[test]
    fn off_screen_left_phases_dont_break_merge_chain() {
        // Viewport t=[30, 130]: phases at 0 and 5 are entirely off-screen left;
        // phases at 10 and 32 are narrow on-screen; phase at 34 is wide.
        // The two on-screen narrow phases must merge — the off-screen phases
        // shouldn't terminate the run.
        let lane = lane(&[(0, true), (5, true), (10, true), (32, true), (34, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(30.0, 130.0), None);
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_merged(&items[0], 10, 2), "{items:?}");
        assert!(is_single(&items[1], 34), "{items:?}");
    }

    #[test]
    fn off_screen_right_phase_stops_iteration() {
        // Viewport t=[0, 100], two visible wide phases, then one off-screen right.
        let lane = lane(&[(0, true), (10, true), (200, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 10), "{items:?}");
    }

    #[test]
    fn trailing_narrow_run_merges_all_but_the_open_ended_last_phase() {
        // 50 narrow phases spaced 2 apart. The chronologically last phase is always pulled
        // out as its own open-ended item, so the first 49 merge and #50 stays separate.
        let phases: Vec<(i64, bool)> = (0..50).map(|i| (i * 2, true)).collect();
        let lane = lane(&phases);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), Some(100.0));
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_merged(&items[0], 0, 49), "{items:?}");
        assert!(is_open_single(&items[1], 98), "{items:?}");
    }

    #[test]
    fn trailing_narrow_run_flushes_when_remaining_phases_are_off_screen_right() {
        // Two narrow phases (50..52, 52..54), then a wide (54..100), then a phase
        // at t=200 that's off-screen-right. The merge group must still be emitted, and the
        // off-screen last phase yields no open-ended item.
        let lane = lane(&[(50, true), (52, true), (54, true), (200, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), None);
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_merged(&items[0], 50, 2), "{items:?}");
        assert!(is_single(&items[1], 54), "{items:?}");
    }

    #[test]
    fn last_phase_is_open_ended_and_extends_to_open_end_time() {
        // Viewport t=[0, 100], one phase at t=0, open_end_time=50 (end of the timeline).
        // The phase is open-ended and ends at x=50 plus the overshoot that tucks it
        // under the zig-zag "end of timeline" band, rather than at the rect edge.
        let lane = lane(&[(0, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0), Some(50.0));
        assert_eq!(items.len(), 1, "{items:?}");
        let RenderItem::Single {
            x_end, end_time, ..
        } = &items[0]
        else {
            panic!("expected Single, got {items:?}");
        };
        assert_eq!(
            *end_time, None,
            "open-ended last phase has no end time: {items:?}"
        );
        assert!(
            (x_end - (50.0 + MAX_ZIG_WIDTH)).abs() < 0.5,
            "x_end={x_end} items={items:?}"
        );
    }

    #[test]
    fn last_phase_starting_at_open_end_has_zero_width_and_is_not_drawn() {
        // Phase starting exactly at open_end_time gets zero width and produces no item.
        // (In the view this doesn't normally happen: open_end_time is the segment's
        // *expanded* end, so a state logged at the last tick keeps a small width.)
        let lane = lane(&[(0, true), (100, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 120.0), Some(100.0));
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
    }
}
