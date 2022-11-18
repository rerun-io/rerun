use crate::ui::view_plot::scene::PlotSeriesKind;
use crate::ViewerContext;
use egui::plot::{Legend, Line, Plot, Points, VLine};
use egui::Color32;
use re_data_store::TimeQuery;

use super::ScenePlot;

// ---

pub(crate) const HELP_TEXT: &str = "Pan by dragging, or scroll (+ shift = horizontal).\n\
    Box zooming: Right click to zoom in and zoom out using a selection.\n\
    Drag with middle mouse button to roll the view.\n\
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

    let time_query = ctx.rec_cfg.time_ctrl.time_query().unwrap();
    let time_type = ctx.rec_cfg.time_ctrl.time_type();

    let x_axis = ctx.rec_cfg.time_ctrl.timeline().name().to_string();
    // Compute the minimum time/X value for the entire plot, so that we can offset everything and
    // avoid nasty precision issues.
    let x_min = scene
        .lines
        .iter()
        .flat_map(|line| line.points.iter().map(|p| p.0))
        .min()
        .unwrap_or(0);

    Plot::new("plot")
        .legend(Legend {
            position: egui::plot::Corner::RightBottom,
            ..Default::default()
        })
        .x_axis_formatter(move |time, _| time_type.format((time as i64 + x_min).into()))
        .label_formatter(move |name, value| {
            let name = if name.is_empty() { "y" } else { name };
            format!(
                "{x_axis}: {}\n{name}: {:.5}",
                time_type.format((value.x as i64 + x_min).into()),
                value.y
            )
        })
        .show(ui, |plot_ui| {
            if plot_ui.plot_secondary_clicked() {
                let timeline = ctx.rec_cfg.time_ctrl.timeline();
                ctx.rec_cfg.time_ctrl.set_timeline_and_time(
                    *timeline,
                    plot_ui.pointer_coordinate().unwrap().x as i64 - x_min,
                );
                ctx.rec_cfg.time_ctrl.pause();
            }

            plot_ui.vline(
                VLine::new(
                    (match time_query {
                        TimeQuery::LatestAt(t) => t,
                        TimeQuery::Range(r) => *r.start(),
                    } - x_min) as f64,
                )
                .color(Color32::WHITE),
            );

            for line in &scene.lines {
                let points = line
                    .points
                    .iter()
                    .map(|p| [(p.0 - x_min) as _, p.1])
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
        })
        .response
}
