use crate::ui::view_plot::scene::PlotSeriesKind;
use crate::ViewerContext;
use egui::plot::{Legend, Line, Plot, Points};
use egui::Color32;
use re_data_store::TimeQuery;

use super::ScenePlot;

// ---

pub(crate) const HELP_TEXT: &str = "Pan by dragging, or scroll (+ shift = horizontal).\n\
    Box zooming: Right click to zoom in and zoom out using a selection.\n\
    Zoom with ctrl / ⌘ + pointer wheel, or with pinch gesture.\n\
    Reset view with double-click.\n\
    Right click to move the time cursor to the current position.";

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ViewPlotState;

pub(crate) fn view_plot(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    _state: &mut ViewPlotState,
    scene: &ScenePlot,
) -> egui::Response {
    crate::profile_function!();

    let time_ctrl = &ctx.rec_cfg.time_ctrl;
    let time_query = time_ctrl.time_query().unwrap();
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
    // large times (nanos since epoch does not fit into a f64).
    let time_offset = min_time;

    // use timeline_name as part of id, so that egui stores different pan/zoom for different timelines
    let plot_id_src = ("plot", &timeline_name);

    let egui::InnerResponse {
        inner: time_x,
        response,
    } = Plot::new(plot_id_src)
        .legend(Legend {
            position: egui::plot::Corner::RightBottom,
            ..Default::default()
        })
        .x_axis_formatter(move |time, _| time_type.format((time as i64 + time_offset).into()))
        .label_formatter(move |name, value| {
            let name = if name.is_empty() { "y" } else { name };
            format!(
                "{timeline_name}: {}\n{name}: {:.5}",
                time_type.format((value.x as i64 + time_offset).into()),
                value.y
            )
        })
        .show(ui, |plot_ui| {
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

            let time_x = (match time_query {
                TimeQuery::LatestAt(t) => t,
                TimeQuery::Range(r) => *r.start(),
            } - time_offset) as f64;

            plot_ui.screen_from_plot([time_x, 0.0].into()).x
        });

    // TODO(emilk): allow interacting with the timeline (may require `egui::Plot` to return the `plot_from_screen` transform)
    let stroke = ui.visuals().widgets.inactive.fg_stroke;
    crate::ui::time_panel::paint_time_cursor(ui.painter(), time_x, response.rect.y_range(), stroke);

    response
}
