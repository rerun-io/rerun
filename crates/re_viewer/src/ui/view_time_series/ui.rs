use egui::{
    plot::{Legend, Line, Plot, Points},
    Color32,
};

use re_arrow_store::TimeType;
use re_time_panel::next_grid_tick_magnitude_ns;
use re_viewer_context::ViewerContext;

use super::SceneTimeSeries;
use crate::ui::{
    spaceview_controls::{
        HORIZONTAL_SCROLL_MODIFIER, MOVE_TIME_CURSOR_BUTTON, RESET_VIEW_BUTTON_TEXT,
        SELECTION_RECT_ZOOM_BUTTON, ZOOM_SCROLL_MODIFIER,
    },
    view_time_series::scene::PlotSeriesKind,
};

// ---

pub fn help_text(re_ui: &re_ui::ReUi) -> egui::WidgetText {
    let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

    layout.add("Pan by dragging, or scroll (+ ");
    layout.add(HORIZONTAL_SCROLL_MODIFIER);
    layout.add(" for horizontal).\n");

    layout.add("Zoom with pinch gesture or scroll + ");
    layout.add(ZOOM_SCROLL_MODIFIER);
    layout.add(".\n");

    layout.add("Drag ");
    layout.add(SELECTION_RECT_ZOOM_BUTTON);
    layout.add(" to zoom in/out using a selection.\n");

    layout.add("Click ");
    layout.add(MOVE_TIME_CURSOR_BUTTON);
    layout.add(" to move the time cursor.\n\n");

    layout.add_button_text(RESET_VIEW_BUTTON_TEXT);
    layout.add(" to reset the view.");

    layout.layout_job.into()
}

#[derive(Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ViewTimeSeriesState;

pub(crate) fn view_time_series(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    _state: &mut ViewTimeSeriesState,
    scene: &SceneTimeSeries,
) -> egui::Response {
    crate::profile_function!();

    let time_ctrl = &ctx.rec_cfg.time_ctrl;
    let current_time = time_ctrl.time_i64();
    let time_type = time_ctrl.time_type();
    let timeline = time_ctrl.timeline();

    let timeline_name = timeline.name().to_string();

    // Compute the minimum time/X value for the entire plot…
    let min_time = scene
        .lines
        .iter()
        .flat_map(|line| line.points.iter().map(|p| p.0))
        .min()
        .unwrap_or(0);

    // …then use that as an offset to avoid nasty precision issues with
    // large times (nanos since epoch does not fit into an f64).
    let time_offset = if timeline.typ() == TimeType::Time {
        // In order to make the tick-marks on the time axis fall on whole days, hours, minutes etc,
        // we need to round to a whole day:
        round_ns_to_start_of_day(min_time)
    } else {
        min_time
    };

    // use timeline_name as part of id, so that egui stores different pan/zoom for different timelines
    let plot_id_src = ("plot", &timeline_name);

    let mut plot = Plot::new(plot_id_src)
        .legend(Legend {
            position: egui::plot::Corner::RightBottom,
            ..Default::default()
        })
        .x_axis_formatter(move |time, _| format_time(time_type, time as i64 + time_offset))
        .label_formatter(move |name, value| {
            let name = if name.is_empty() { "y" } else { name };
            let is_integer = value.y.round() == value.y;
            let decimals = if is_integer { 0 } else { 5 };
            format!(
                "{timeline_name}: {}\n{name}: {:.*}",
                time_type.format((value.x as i64 + time_offset).into()),
                decimals,
                value.y,
            )
        });

    if timeline.typ() == TimeType::Time {
        let canvas_size = ui.available_size();
        plot = plot.x_grid_spacer(move |spacer| ns_grid_spacer(canvas_size, &spacer));
    }

    let egui::plot::PlotResponse {
        inner: time_x,
        response,
        transform,
    } = plot.show(ui, |plot_ui| {
        if plot_ui.plot_secondary_clicked() {
            let timeline = ctx.rec_cfg.time_ctrl.timeline();
            ctx.rec_cfg.time_ctrl.set_timeline_and_time(
                *timeline,
                plot_ui.pointer_coordinate().unwrap().x as i64 + time_offset,
            );
            ctx.rec_cfg.time_ctrl.pause();
        }

        for line in &scene.lines {
            let points = line
                .points
                .iter()
                .map(|p| [(p.0 - time_offset) as _, p.1])
                .collect::<Vec<_>>();

            let c = line.color;
            let color = Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);

            match line.kind {
                PlotSeriesKind::Continuous => plot_ui.line(
                    Line::new(points)
                        .name(&line.label)
                        .color(color)
                        .width(line.width),
                ),
                PlotSeriesKind::Scatter => plot_ui.points(
                    Points::new(points)
                        .name(&line.label)
                        .color(color)
                        .radius(line.width),
                ),
            }
        }

        current_time.map(|current_time| {
            let time_x = (current_time - time_offset) as f64;
            plot_ui.screen_from_plot([time_x, 0.0].into()).x
        })
    });

    if let Some(time_x) = time_x {
        let interact_radius = ui.style().interaction.resize_grab_radius_side;
        let line_rect = egui::Rect::from_x_y_ranges(time_x..=time_x, response.rect.y_range())
            .expand(interact_radius);

        let time_drag_id = ui.id().with("time_drag");
        let response = ui
            .interact(line_rect, time_drag_id, egui::Sense::drag())
            .on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);

        if response.dragged() {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                let time =
                    time_offset + transform.value_from_position(pointer_pos).x.round() as i64;

                let time_ctrl = &mut ctx.rec_cfg.time_ctrl;
                time_ctrl.set_time(time);
                time_ctrl.pause();
            }
        }

        let stroke = if response.dragged() {
            ui.style().visuals.widgets.active.fg_stroke
        } else if response.hovered() {
            ui.style().visuals.widgets.hovered.fg_stroke
        } else {
            ui.visuals().widgets.inactive.fg_stroke
        };
        re_time_panel::paint_time_cursor(ui.painter(), time_x, response.rect.y_range(), stroke);
    }

    response
}

fn format_time(time_type: TimeType, time_int: i64) -> String {
    if time_type == TimeType::Time {
        let time = re_log_types::Time::from_ns_since_epoch(time_int);
        re_time_panel::format_time_compact(time)
    } else {
        time_type.format(re_log_types::TimeInt::from(time_int))
    }
}

fn ns_grid_spacer(
    canvas_size: egui::Vec2,
    input: &egui::plot::GridInput,
) -> Vec<egui::plot::GridMark> {
    let minimum_medium_line_spacing = 150.0; // ≈min size of a label
    let max_medium_lines = canvas_size.x as f64 / minimum_medium_line_spacing;

    let (min_ns, max_ns) = input.bounds;
    let width_ns = max_ns - min_ns;

    let mut small_spacing_ns = 1;
    while width_ns / (next_grid_tick_magnitude_ns(small_spacing_ns) as f64) > max_medium_lines {
        small_spacing_ns = next_grid_tick_magnitude_ns(small_spacing_ns);
    }
    let medium_spacing_ns = next_grid_tick_magnitude_ns(small_spacing_ns);
    let big_spacing_ns = next_grid_tick_magnitude_ns(medium_spacing_ns);

    let mut current_ns = (min_ns.floor() as i64) / small_spacing_ns * small_spacing_ns;
    let mut marks = vec![];

    while current_ns <= max_ns.ceil() as i64 {
        let is_big_line = current_ns % big_spacing_ns == 0;
        let is_medium_line = current_ns % medium_spacing_ns == 0;

        let step_size = if is_big_line {
            big_spacing_ns
        } else if is_medium_line {
            medium_spacing_ns
        } else {
            small_spacing_ns
        };

        marks.push(egui::plot::GridMark {
            value: current_ns as f64,
            step_size: step_size as f64,
        });

        current_ns += small_spacing_ns;
    }

    marks
}

fn round_ns_to_start_of_day(ns: i64) -> i64 {
    let ns_per_day = 24 * 60 * 60 * 1_000_000_000;
    (ns + ns_per_day / 2) / ns_per_day * ns_per_day
}
