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

    // TODO:
    // - plug in Legend/ClassDescr?
    //
    // A scalar _literally_ cannot be timeless: we wouldn't even have an x value to work with!
    //
    // - what does the timequery look like?
    //   we're always timed and always sticky,
    // - what happens when the scalar changes color?
    // - what happens when the scalar changes label?
    //
    // what about stuff that has nothing to do with points, e.g. the kind of plot, or whether
    // we want a reference hline/vline to appear? Or better: stickiness!
    // - Sometimes it's nice to still set it at the scalar-level, so that things can evolve
    //   over time.
    // - on the other hand, do you really want each scalar to reassert the fact that this is
    //   a bar chart or whatever? (whether from a logic perspective, or a storage perspective)
    //
    // what happens when you want to set one color on the scalar itself, and another for line
    // that goes through all of these scalars?
    //
    // Shall these things use some kind of metadata API that applies to a whole obj_path,
    // similar to class descriptions? E.g. we expose some kind of `log_plot_config`?
    // Does this tie in with Jeremy's work on annotation contexts?
    //
    // Or should it all be handled by blueprints somehow..? Or both?!
    //
    // - Label of a line is a good example of something that shouldn't be derived from points'

    let tq = ctx.rec_cfg.time_ctrl.time_query().unwrap();

    let x_axis = ctx.rec_cfg.time_ctrl.timeline().name().to_string();

    Plot::new("plot") // TODO
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
                VLine::new(match tq {
                    TimeQuery::LatestAt(t) => t as f64,
                    TimeQuery::Range(r) => *r.start() as f64,
                })
                .color(Color32::WHITE),
            );

            for line in &scene.lines {
                let points = line
                    .points
                    .iter()
                    .map(|p| [p.time as _, p.value])
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
