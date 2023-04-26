use egui::{
    plot::{Legend, Line, Plot, Points},
    Color32,
};

use re_arrow_store::TimeType;

use crate::{
    misc::format_time::next_grid_tick_magnitude_ns, ui::view_time_series::scene::PlotSeriesKind,
    ViewerContext,
};

use super::SceneTimeSeries;

// ---

pub(crate) const HELP_TEXT: &str = "Pan by dragging, or scroll (+ shift = horizontal).\n\
    Box zooming: Right click to zoom in and zoom out using a selection.\n\
    Zoom with ctrl / ⌘ + pointer wheel, or with pinch gesture.\n\
    Reset view with double-click.\n\
    Right click to move the time cursor to the current position.";

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
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
        transform: _,
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
        // TODO(emilk): allow interacting with the timeline (may require `egui::Plot` to return the `plot_from_screen` transform)
        let stroke = ui.visuals().widgets.inactive.fg_stroke;
        crate::ui::time_panel::paint_time_cursor(
            ui.painter(),
            time_x,
            response.rect.y_range(),
            stroke,
        );
    }

    response
}

fn format_time(time_type: TimeType, time_int: i64) -> String {
    if time_type == TimeType::Time {
        let time = re_log_types::Time::from_ns_since_epoch(time_int);
        crate::misc::format_time::format_time_compact(time)
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
