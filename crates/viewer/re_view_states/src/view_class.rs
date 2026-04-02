use re_log_types::EntityPath;
use re_ui::{Help, icons};
use re_viewer_context::{
    IdentifiedViewSystem as _, IndicatedEntities, PerVisualizerType, RecommendedVisualizers,
    ViewClass, ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemIdentifier, ViewerContext, VisualizableReason,
};

use crate::data::{StateLane, StateLanesData};

// Layout constants (in screen pixels).
const LANE_BAND_HEIGHT: f32 = 22.0;
const LANE_LABEL_HEIGHT: f32 = 14.0;
const LANE_GAP: f32 = 4.0;
const LANE_TOTAL_HEIGHT: f32 = LANE_BAND_HEIGHT + LANE_LABEL_HEIGHT + LANE_GAP;

const TIME_AXIS_HEIGHT: f32 = 20.0;
const TOP_MARGIN: f32 = 4.0;

/// View state for pan/zoom.
#[derive(Default)]
struct StatesViewState {
    /// Visible time range: (min, max) in timeline units.
    /// `None` means "fit all data".
    time_range: Option<(f64, f64)>,
}

impl ViewState for StatesViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct StatesView;

impl ViewClass for StatesView {
    fn identifier() -> re_sdk_types::ViewClassIdentifier {
        "States".into()
    }

    fn display_name(&self) -> &'static str {
        "States"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        // TODO(RR-4264): Add a proper icon.
        &icons::VIEW_GENERIC
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<StatesViewState>::default()
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("States view")
            .markdown("Shows state transitions as horizontal colored lanes over time.")
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<crate::StatesVisualizer>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        Some(2.5)
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        _ctx: &ViewerContext<'_>,
        _include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        // TODO(RR-4248): Spawn heuristics for state logs.
        ViewSpawnHeuristics::empty()
    }

    fn recommended_visualizers_for_entity(
        &self,
        _entity_path: &EntityPath,
        visualizers: &[(ViewSystemIdentifier, &VisualizableReason)],
        _indicated_entities_per_visualizer: &PerVisualizerType<&IndicatedEntities>,
    ) -> RecommendedVisualizers {
        if visualizers
            .iter()
            .any(|(viz, _)| *viz == crate::StatesVisualizer::identifier())
        {
            RecommendedVisualizers::default(crate::StatesVisualizer::identifier())
        } else {
            RecommendedVisualizers::empty()
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

        let state = state.downcast_mut::<StatesViewState>()?;

        // Collect all lanes from all visualizers.
        let all_lanes: Vec<&StateLane> = system_output
            .iter_visualizer_data::<StateLanesData>()
            .flat_map(|d| d.lanes.iter())
            .collect();

        if all_lanes.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No state data. Add a visualizer that produces StateLanesData.");
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
        if state.time_range.is_none() {
            let padding = (data_max - data_min).max(1.0) * 0.05;
            state.time_range = Some((data_min - padding, data_max + padding));
        }

        let Some((mut t_min, mut t_max)) = state.time_range else {
            return Ok(());
        };

        // Allocate the full available rect.
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        if !ui.is_rect_visible(rect) {
            return Ok(());
        }

        // Handle select / hover on the view itself.
        ctx.handle_select_hover_drag_interactions(
            &response,
            re_viewer_context::Item::View(query.view_id),
            false,
        );

        // Lane drawing area (above the time axis).
        let lanes_rect = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.right(), rect.bottom() - TIME_AXIS_HEIGHT),
        );
        let time_axis_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - TIME_AXIS_HEIGHT),
            rect.right_bottom(),
        );

        // Pan & zoom.
        handle_pan_zoom(ui, &response, lanes_rect, &mut t_min, &mut t_max);
        state.time_range = Some((t_min, t_max));

        // Background.
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.style().visuals.extreme_bg_color);

        // Draw lanes.
        let label_color = ui.style().visuals.text_color();
        let weak_color = ui.style().visuals.weak_text_color();
        for (lane_idx, lane) in all_lanes.iter().enumerate() {
            paint_lane(
                &painter,
                lanes_rect,
                lane_idx,
                lane,
                t_min,
                t_max,
                label_color,
            );
        }

        // Draw time axis.
        paint_time_axis(
            &painter,
            time_axis_rect,
            t_min,
            t_max,
            label_color,
            weak_color,
        );

        // Draw time cursor as a full-height vertical line.
        let current_time = query.latest_at.as_i64() as f64;
        if current_time >= t_min && current_time <= t_max {
            let cursor_x = time_to_x(current_time, rect, t_min, t_max);
            let stroke = ui.visuals().widgets.inactive.fg_stroke;
            painter.vline(
                cursor_x,
                rect.top()..=rect.bottom(),
                egui::Stroke::new(1.5 * stroke.width, stroke.color),
            );
        }

        Ok(())
    }
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

/// Map a time value to screen x within the given rect.
fn time_to_x(t: f64, rect: egui::Rect, t_min: f64, t_max: f64) -> f32 {
    let frac = ((t - t_min) / (t_max - t_min)) as f32;
    egui::lerp(rect.left()..=rect.right(), frac)
}

/// Handle drag-to-pan and scroll-to-zoom interactions.
fn handle_pan_zoom(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    t_min: &mut f64,
    t_max: &mut f64,
) {
    let range = *t_max - *t_min;

    // Drag to pan.
    if response.dragged() {
        let dx = response.drag_delta().x;
        let dt = -(dx as f64 / rect.width() as f64) * range;
        *t_min += dt;
        *t_max += dt;
    }

    // Scroll to zoom.
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll.abs() > 0.0 && response.hovered() {
        let zoom_factor = (scroll / 200.0).exp() as f64;
        // Zoom centered on the mouse position.
        let mouse_x = ui
            .input(|i| i.pointer.hover_pos())
            .map(|p| p.x)
            .unwrap_or_else(|| rect.center().x);
        let frac = ((mouse_x - rect.left()) / rect.width()) as f64;
        let pivot = *t_min + frac * range;

        *t_min = pivot - (pivot - *t_min) / zoom_factor;
        *t_max = pivot + (*t_max - pivot) / zoom_factor;
    }
}

/// Paint a single lane (label + colored band of phases).
fn paint_lane(
    painter: &egui::Painter,
    lanes_rect: egui::Rect,
    lane_idx: usize,
    lane: &StateLane,
    t_min: f64,
    t_max: f64,
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

    // Phases.
    for (i, phase) in lane.phases.iter().enumerate() {
        let x_start = time_to_x(phase.start_time as f64, lanes_rect, t_min, t_max);
        let x_end = if let Some(next) = lane.phases.get(i + 1) {
            time_to_x(next.start_time as f64, lanes_rect, t_min, t_max)
        } else {
            lanes_rect.right()
        };

        // Clip to visible area.
        let x_start = x_start.max(lanes_rect.left());
        let x_end = x_end.min(lanes_rect.right());

        if x_end <= x_start {
            continue;
        }

        let phase_rect = egui::Rect::from_min_max(
            egui::pos2(x_start, band_y_top),
            egui::pos2(x_end, band_y_bottom),
        );

        painter.add(egui::epaint::RectShape::new(
            phase_rect,
            0.0,
            phase.color,
            egui::Stroke::NONE,
            egui::StrokeKind::Outside,
        ));

        // Phase label (clipped to band width).
        let text_width = x_end - x_start - 6.0;
        if text_width > 10.0 {
            painter.with_clip_rect(phase_rect).text(
                egui::pos2(x_start + 4.0, band_y_top + 3.0),
                egui::Align2::LEFT_TOP,
                &phase.label,
                egui::FontId::proportional(12.0),
                readable_text_color(phase.color),
            );
        }
    }
}

/// Choose white or black text depending on background luminance.
fn readable_text_color(bg: egui::Color32) -> egui::Color32 {
    if bg.intensity() > 0.6 {
        egui::Color32::BLACK
    } else {
        egui::Color32::WHITE
    }
}

/// Paint the time axis with tick marks and labels.
fn paint_time_axis(
    painter: &egui::Painter,
    rect: egui::Rect,
    t_min: f64,
    t_max: f64,
    text_color: egui::Color32,
    weak_color: egui::Color32,
) {
    let range = t_max - t_min;
    if range <= 0.0 {
        return;
    }

    // Separator line.
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, weak_color),
    );

    // Compute a nice tick spacing.
    let approx_num_ticks = (rect.width() / 80.0).max(2.0) as usize;
    let raw_step = range / approx_num_ticks as f64;
    let magnitude = 10.0_f64.powf(raw_step.log10().floor());
    let residual = raw_step / magnitude;
    let step = if residual <= 1.5 {
        magnitude
    } else if residual <= 3.5 {
        2.0 * magnitude
    } else if residual <= 7.5 {
        5.0 * magnitude
    } else {
        10.0 * magnitude
    };

    let first_tick = (t_min / step).ceil() * step;
    let mut t = first_tick;
    while t <= t_max {
        let x = time_to_x(t, rect, t_min, t_max);

        // Tick mark.
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.top() + 4.0)],
            egui::Stroke::new(1.0, weak_color),
        );

        // Label.
        let label = if step >= 1.0 {
            format!("{}", t as i64)
        } else {
            format!("{t:.1}")
        };
        painter.text(
            egui::pos2(x, rect.top() + 5.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(10.0),
            text_color,
        );

        t += step;
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| StatesView.help(ctx));
}
