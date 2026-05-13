use re_log_types::{
    AbsoluteTimeRange, EntityPath, TimeCell, TimeInt, TimeReal, TimeType, TimelineName,
    TimestampFormat,
};
use re_time_ruler::TimeRangesUi;
use re_ui::{Help, UiExt as _, icons};
use re_viewer_context::{
    DataResultInteractionAddress, IdentifiedViewSystem as _, Item, TimeControlCommand, TimeView,
    ViewClass, ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
};

use crate::data::{StateLane, StateLanePhase, StateLanesData};

// Layout constants (in screen pixels).
const LANE_BAND_HEIGHT: f32 = 22.0;
const LANE_LABEL_HEIGHT: f32 = 14.0;
const LANE_GAP: f32 = 4.0;
const LANE_TOTAL_HEIGHT: f32 = LANE_BAND_HEIGHT + LANE_LABEL_HEIGHT + LANE_GAP;

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

        /// End time of the phase (start of the next phase), if any.
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
#[derive(Default)]
struct StateTimelineViewState {
    /// Visible time range, in the same representation as the timeline panel.
    /// `None` means "fit all data" — populated on the next frame from the data range.
    time_view: Option<TimeView>,

    /// The timeline we last rendered. When the active timeline changes,
    /// we reset `time_view` so the view auto-fits to the new data.
    active_timeline: Option<TimelineName>,

    /// `true` if the current primary-button press landed on a phase rectangle.
    /// A phase press selects the phase's entity and does NOT move the time cursor;
    /// a press on empty space drags the time cursor.
    press_on_phase: bool,
}

impl re_byte_size::SizeBytes for StateTimelineViewState {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            time_view: _,
            active_timeline,
            press_on_phase: _,
        } = self;

        active_timeline.heap_size_bytes()
    }
}

impl ViewState for StateTimelineViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct StateTimelineView;

impl ViewClass for StateTimelineView {
    fn identifier() -> re_sdk_types::ViewClassIdentifier {
        "StateTimeline".into()
    }

    fn display_name(&self) -> &'static str {
        "State timeline"
    }

    // TODO(RR-4506): Remove this function once the State Timeline view graduates from experimental.
    fn is_experimental(&self) -> bool {
        true
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &icons::VIEW_STATE_TIMELINE
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<StateTimelineViewState>::default()
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("State timeline view")
            .markdown("Shows state transitions as horizontal colored lanes over time.")
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
        _ctx: &ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        Ok(())
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

        // Reset the view when the active timeline changes.
        if state.active_timeline.as_ref() != Some(&query.timeline) {
            state.active_timeline = Some(query.timeline);
            state.time_view = None;
        }

        // Collect all lanes from all visualizers.
        let all_lanes: Vec<&StateLane> = system_output
            .iter_visualizer_data::<StateLanesData>()
            .flat_map(|d| d.lanes.iter())
            .collect();

        if all_lanes.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No state change data. Add a visualizer that produces StateLanesData.");
            });
            return Ok(());
        }

        // Compute data time range.
        let (data_min, data_max) = data_time_range(&all_lanes);

        // Auto-fit on first frame.
        // TODO(aedm): The calculation of the end time is incorrect since state transitions don't have an end time.
        //      We should use an estimation so that the latest state is still somewhat visible. Maybe also consider
        //      the density of states? An idea is to keep as much space for the last state as the average state
        //      duration on the screen.
        if state.time_view.is_none() {
            let padding = (data_max - data_min).max(1.0) * 0.05;
            let min = data_min - padding;
            let max = data_max + padding;
            state.time_view = Some(TimeView {
                min: TimeReal::from(min),
                time_spanned: max - min,
            });
        }

        let Some(mut time_view) = state.time_view else {
            return Ok(());
        };

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

        let current_time = TimeReal::from(query.latest_at.as_i64() as f64);
        let cursor_x = time_ranges_ui.x_from_time_f32(current_time);

        // On primary press, remember whether it landed on a phase. A phase press
        // selects the entity; a press on empty space drags the time cursor.
        if ui.input(|i| i.pointer.primary_pressed()) {
            state.press_on_phase = response
                .interact_pointer_pos()
                .is_some_and(|pos| hit_test_phase(pos, lanes_rect, &all_lanes, &time_ranges_ui));
        }

        // While the primary button is active on the view and the press started on
        // empty space, move the time cursor to the pointer. Using primary_pressed /
        // primary_down / primary_released mirrors `re_time_panel` so that the cursor
        // jumps on press and then follows during a drag.
        let primary_active = response.hovered()
            && ui.input(|i| {
                i.pointer.primary_pressed()
                    || i.pointer.primary_down()
                    || i.pointer.primary_released()
            });
        let dragging_cursor = primary_active && !state.press_on_phase;
        if dragging_cursor
            && let Some(pos) = response.interact_pointer_pos()
            && let Some(time) = time_ranges_ui.time_from_x_f32(pos.x)
        {
            ctx.send_time_commands([TimeControlCommand::Pause, TimeControlCommand::SetTime(time)]);
        }

        // Pan: right- or middle-click drag, plus two-finger touchpad horizontal scroll.
        // Cmd+scroll is routed to `zoom_delta` by egui, so it won't double-fire here.
        let mut pan_dx = 0.0;
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            pan_dx += response.drag_delta().x;
            ui.ctx().set_cursor_icon(egui::CursorIcon::AllScroll);
        }
        if response.hovered() {
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
            && response.hovered()
            && let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos())
            && let Some(new_view) = time_ranges_ui.zoom_at(pointer_pos.x, zoom_delta)
        {
            time_view = new_view;
        }
        state.time_view = Some(time_view);

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

        // Draw lanes.
        let label_color = ui.style().visuals.text_color();
        for (lane_idx, lane) in all_lanes.iter().enumerate() {
            paint_lane(
                ui,
                &painter,
                lanes_rect,
                lane_idx,
                lane,
                &time_ranges_ui,
                time_type,
                timestamp_format,
                label_color,
            );
        }

        // Handle selection: determine what's under the pointer (lane entity or view).
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let hovered_lane = hover_pos.and_then(|pos| hovered_lane(pos, lanes_rect, &all_lanes));

        // Time cursor — uses the same triangle-headed style as the time panel.
        if let Some(cursor_x) = cursor_x
            && rect.x_range().contains(cursor_x)
        {
            let cursor_response = if dragging_cursor || hovered_lane.is_none() {
                Some(&response)
            } else {
                None
            };
            ui.paint_time_cursor(&painter, cursor_response, cursor_x, rect.y_range());
        }

        let interacted_item = if let Some(entity_path) = hovered_lane {
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
    lane: &'a StateLane,
    lanes_rect: egui::Rect,
    time_ranges_ui: &TimeRangesUi,
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

    for (i, phase) in lane.phases.iter().enumerate() {
        // Invisible phases create a gap; they must not be merged across.
        if !phase.visible {
            pending.flush(&mut items);
            continue;
        }

        let next_start_time = lane.phases.get(i + 1).map(|p| p.start_time);
        let Some(x_start) = time_ranges_ui.x_from_time_f32(TimeReal::from(phase.start_time as f64))
        else {
            continue;
        };
        let x_end_unclipped = match next_start_time {
            Some(t) => time_ranges_ui
                .x_from_time_f32(TimeReal::from(t as f64))
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

        if width >= MERGE_PHASE_THRESHOLD_PIXEL {
            pending.flush(&mut items);
            items.push(RenderItem::Single {
                phase,
                x_start: visible_x_start,
                x_end: visible_x_end,
                end_time: next_start_time,
            });
        } else {
            pending.push(PendingNarrow {
                phase,
                x_start: visible_x_start,
                x_end: visible_x_end,
                end_time: next_start_time,
            });
        }
    }
    pending.flush(&mut items);

    items
}

/// Compute the (min, max) time range across all lanes.
fn data_time_range(lanes: &[&StateLane]) -> (f64, f64) {
    let mut min = f64::MAX;
    let mut max = f64::MIN;
    for lane in lanes {
        for phase in &lane.phases {
            let t = phase.start_time as f64;
            min = min.min(t);
            max = max.max(t);
        }
    }
    if min > max {
        (0.0, 1.0)
    } else if (max - min).abs() < f64::EPSILON {
        (min - 0.5, max + 0.5)
    } else {
        (min, max)
    }
}

/// Returns the entity path of the lane under `pos`, if any.
fn hovered_lane<'a>(
    pos: egui::Pos2,
    lanes_rect: egui::Rect,
    lanes: &'a [&'a StateLane],
) -> Option<&'a EntityPath> {
    lanes.iter().enumerate().find_map(|(lane_idx, lane)| {
        let y_top =
            lanes_rect.top() + TOP_MARGIN + lane_idx as f32 * LANE_TOTAL_HEIGHT + LANE_LABEL_HEIGHT;
        let y_bottom = y_top + LANE_BAND_HEIGHT;
        (pos.y >= y_top && pos.y <= y_bottom).then_some(&lane.entity_path)
    })
}

/// Returns `true` if `pos` lies inside any visible phase rectangle.
fn hit_test_phase(
    pos: egui::Pos2,
    lanes_rect: egui::Rect,
    lanes: &[&StateLane],
    time_ranges_ui: &TimeRangesUi,
) -> bool {
    for (lane_idx, lane) in lanes.iter().enumerate() {
        let y_top = lanes_rect.top() + TOP_MARGIN + lane_idx as f32 * LANE_TOTAL_HEIGHT;
        let band_y_top = y_top + LANE_LABEL_HEIGHT;
        let band_y_bottom = band_y_top + LANE_BAND_HEIGHT;
        if pos.y < band_y_top || pos.y > band_y_bottom {
            continue;
        }
        for (i, phase) in lane.phases.iter().enumerate() {
            if !phase.visible {
                continue;
            }
            let Some(x_start) =
                time_ranges_ui.x_from_time_f32(TimeReal::from(phase.start_time as f64))
            else {
                continue;
            };
            let x_start = x_start.max(lanes_rect.left());
            let x_end = if let Some(next) = lane.phases.get(i + 1) {
                time_ranges_ui
                    .x_from_time_f32(TimeReal::from(next.start_time as f64))
                    .unwrap_or_else(|| lanes_rect.right())
            } else {
                lanes_rect.right()
            }
            .min(lanes_rect.right());
            if x_end <= x_start {
                continue;
            }
            if pos.x >= x_start && pos.x <= x_end {
                return true;
            }
        }
    }
    false
}

/// Paint a single lane (label + colored band of phases) and show tooltips on hover.
#[expect(clippy::too_many_arguments)]
fn paint_lane(
    ui: &egui::Ui,
    painter: &egui::Painter,
    lanes_rect: egui::Rect,
    lane_idx: usize,
    lane: &StateLane,
    time_ranges_ui: &TimeRangesUi,
    time_type: TimeType,
    timestamp_format: TimestampFormat,
    label_color: egui::Color32,
) {
    let y_top = lanes_rect.top() + TOP_MARGIN + lane_idx as f32 * LANE_TOTAL_HEIGHT;
    let label_rect = egui::Rect::from_min_size(
        egui::pos2(lanes_rect.left() + 4.0, y_top),
        egui::vec2(lanes_rect.width() - 8.0, LANE_LABEL_HEIGHT),
    );
    let band_y_top = y_top + LANE_LABEL_HEIGHT;
    let band_y_bottom = band_y_top + LANE_BAND_HEIGHT;

    // Lane label.
    painter.text(
        label_rect.left_top(),
        egui::Align2::LEFT_TOP,
        &lane.label,
        egui::FontId::proportional(11.0),
        label_color,
    );

    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let render_items = compute_render_items(lane, lanes_rect, time_ranges_ui);

    let merged_fill_inactive = ui.visuals().widgets.inactive.bg_fill;
    let merged_fill_hovered = ui.visuals().widgets.hovered.bg_fill;
    let merged_text_color = ui.visuals().text_color();

    for item in &render_items {
        let (x_start, x_end) = item.x_range();
        let item_rect = egui::Rect::from_min_max(
            egui::pos2(x_start, band_y_top),
            egui::pos2(x_end, band_y_bottom),
        );
        let hovered = hover_pos.is_some_and(|pos| item_rect.contains(pos));

        match item {
            RenderItem::Single { phase, .. } => paint_single(painter, item_rect, phase, hovered),
            RenderItem::Merged { count, .. } => {
                let fill = if hovered {
                    merged_fill_hovered
                } else {
                    merged_fill_inactive
                };
                paint_merged(painter, item_rect, *count, fill, merged_text_color);
            }
        }

        if let Some(pos) = hover_pos
            && item_rect.contains(pos)
        {
            show_item_tooltip(ui, item, time_type, timestamp_format);
        }
    }
}

/// Paint one normal phase: filled band (dimmed when not hovered) + clipped label.
fn paint_single(painter: &egui::Painter, rect: egui::Rect, phase: &StateLanePhase, hovered: bool) {
    #[expect(clippy::disallowed_methods)] // Data-driven visualization color, not a UI theme color.
    let fill = if hovered {
        phase.color
    } else {
        let [r, g, b, _] = phase.color.to_array();
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
            &phase.label,
            egui::FontId::proportional(12.0),
            readable_text_color(phase.color),
        );
    }
}

/// Paint a merged region: a flat band in a theme widget color signaling that many
/// narrow phases have been collapsed at the current zoom level. The caller picks the
/// fill from `widgets.inactive`/`widgets.hovered` so the hover state stays
/// token-driven rather than relying on an arbitrary multiplier.
fn paint_merged(
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
        match item {
            RenderItem::Single {
                phase, end_time, ..
            } => {
                ui.label(&phase.label);
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
    use re_log_types::EntityPath;

    /// Construct a `StateLane` from `(start_time, visible)` pairs. Color/label are
    /// unused by `compute_render_items`, so we leave them dummy.
    fn lane(phases: &[(i64, bool)]) -> StateLane {
        StateLane {
            label: "test".into(),
            entity_path: EntityPath::from("/test"),
            phases: phases
                .iter()
                .map(|&(t, visible)| StateLanePhase {
                    start_time: t,
                    label: String::new(),
                    color: egui::Color32::TRANSPARENT,
                    visible,
                })
                .collect(),
        }
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
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert!(items.is_empty(), "{items:?}");
    }

    #[test]
    fn single_wide_phase_renders_as_single() {
        // One phase covering x=0..100 — well above the merge threshold.
        let lane = lane(&[(0, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
    }

    #[test]
    fn lone_narrow_phase_renders_as_single_not_merged() {
        // Phase 0: x=0..2 (narrow). Phase 1: x=2..100 (wide).
        // The narrow phase has no narrow neighbor to merge with, so it stays Single.
        let lane = lane(&[(0, true), (2, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 2), "{items:?}");
    }

    #[test]
    fn two_consecutive_narrow_phases_merge() {
        // Two narrow (x=0..2, 2..4) + one wide (x=4..100).
        let lane = lane(&[(0, true), (2, true), (4, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_merged(&items[0], 0, 2), "{items:?}");
        assert!(is_single(&items[1], 4), "{items:?}");
    }

    #[test]
    fn wide_phase_breaks_merge_chain() {
        // Wide (0..10), narrow (10..12), wide (12..100) — the lone narrow stays Single.
        let lane = lane(&[(0, true), (10, true), (12, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
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
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert_eq!(items.len(), 3, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 4), "{items:?}");
        assert!(is_single(&items[2], 6), "{items:?}");
    }

    #[test]
    fn off_screen_left_phases_dont_break_merge_chain() {
        // Viewport t=[30, 130]: phases at 0 and 5 are entirely off-screen left;
        // phases at 10 and 32 are narrow on-screen; phase at 34 is wide.
        // The two on-screen narrow phases must merge — the off-screen phases
        // shouldn't terminate the run.
        let lane = lane(&[(0, true), (5, true), (10, true), (32, true), (34, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(30.0, 130.0));
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_merged(&items[0], 10, 2), "{items:?}");
        assert!(is_single(&items[1], 34), "{items:?}");
    }

    #[test]
    fn off_screen_right_phase_stops_iteration() {
        // Viewport t=[0, 100], two visible wide phases, then one off-screen right.
        let lane = lane(&[(0, true), (10, true), (200, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_single(&items[0], 0), "{items:?}");
        assert!(is_single(&items[1], 10), "{items:?}");
    }

    #[test]
    fn trailing_narrow_run_flushes_as_merged_after_loop() {
        // 50 narrow phases spaced 2 apart, covering the entire visible range.
        // Verifies that the post-loop flush emits the Merged region (no wide phase
        // forces an earlier flush).
        let phases: Vec<(i64, bool)> = (0..50).map(|i| (i * 2, true)).collect();
        let lane = lane(&phases);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert_eq!(items.len(), 1, "{items:?}");
        assert!(is_merged(&items[0], 0, 50), "{items:?}");
    }

    #[test]
    fn trailing_narrow_run_flushes_when_remaining_phases_are_off_screen_right() {
        // Two narrow phases (50..52, 52..54), then a wide (54..100), then a phase
        // at t=200 that's off-screen-right. The merge group must still be emitted.
        let lane = lane(&[(50, true), (52, true), (54, true), (200, true)]);
        let items = compute_render_items(&lane, unit_rect(), &ranges_ui(0.0, 100.0));
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(is_merged(&items[0], 50, 2), "{items:?}");
        assert!(is_single(&items[1], 54), "{items:?}");
    }
}
