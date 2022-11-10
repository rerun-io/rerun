use crate::ui::view_plot::scene::PlotLineKind;
use crate::ViewerContext;
use egui::plot::{Legend, Line, Plot, Points, VLine};
use egui::Color32;
use re_data_store::TimeQuery;

use super::ScenePlot;

// ---

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
    let x_axis = ctx.rec_cfg.time_ctrl.timeline().name().to_string();

    Plot::new("plot")
        .legend(Legend::default())
        .label_formatter(move |name, value| {
            let name = if name.is_empty() { "y" } else { name };
            format!("{x_axis}: {:.0}\n{name}: {:.0}", value.x, value.y)
        })
        .show(ui, |plot_ui| {
            if plot_ui.plot_clicked() {
                let timeline = ctx.rec_cfg.time_ctrl.timeline();
                ctx.rec_cfg
                    .time_ctrl
                    .set_timeline_and_time(*timeline, plot_ui.pointer_coordinate().unwrap().x);
                ctx.rec_cfg.time_ctrl.pause();
            }

            plot_ui.vline(
                VLine::new(match time_query {
                    TimeQuery::LatestAt(t) => t as f64,
                    TimeQuery::Range(r) => *r.start() as f64,
                })
                .color(Color32::WHITE),
            );

            for line in &scene.lines {
                let points = line
                    .points
                    .iter()
                    .map(|p| [p.0 as _, p.1])
                    .collect::<Vec<_>>();

                let c = line.color;
                let color = Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);

                match line.kind {
                    PlotLineKind::Continuous => plot_ui.line(
                        Line::new(points)
                            .name(&line.label)
                            .color(color)
                            .width(line.width),
                    ),
                    PlotLineKind::Scatter => plot_ui.points(
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
