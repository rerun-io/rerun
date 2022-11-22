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

    let time_query = ctx.rec_cfg.time_ctrl.time_query().unwrap();
    let time_type = ctx.rec_cfg.time_ctrl.time_type();

    let x_axis = ctx.rec_cfg.time_ctrl.timeline().name().to_string();

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

    let egui::InnerResponse {
        inner: time_x,
        response,
    } = Plot::new("plot")
        .legend(Legend {
            position: egui::plot::Corner::RightBottom,
            ..Default::default()
        })
        .x_axis_formatter(move |time, _| time_type.format((time as i64 + time_offset).into()))
        .label_formatter(move |name, value| {
            let name = if name.is_empty() { "y" } else { name };
            format!(
                "{x_axis}: {}\n{name}: {:.5}",
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

    {
        // We paint the time explicitly (not using plot::VLine) so that
        // A) the time vline isn't part of the calculation when computing automatic bounds for the plot
        // B) we can round to nearest pixel to reduce aliasing when time moves
        let x = ui.painter().round_to_pixel(time_x);
        let y = response.rect.y_range();
        ui.painter().vline(x, y, (1.0, Color32::WHITE));
    }

    response
}
