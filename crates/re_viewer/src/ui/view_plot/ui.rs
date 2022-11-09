use crate::ui::view_plot::scene::PlotLineKind;
use crate::ViewerContext;
use egui::plot::{Legend, Line, Plot, PlotPoints, Points};
use egui::Color32;
use re_log_types::{LogMsg, TimePoint};

use super::ScenePlot;

// ---

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewPlotState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// plot entry window however they please when the time cursor isn't moving.
    latest_time: i64,
}

pub(crate) fn view_plot(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewPlotState,
    scene: &ScenePlot,
) -> egui::Response {
    crate::profile_function!();

    // TODO:
    // - x legend (using timeline name)
    // - y legend (using obj_path or label if available)
    // - plug in Legend/ClassDescr?
    // - position marker
    // - log_scalars doesn't make sense!
    // - does the time thing behave correctly here? what about multi-timeline setups?
    // - document how spaces work for plots
    // - vertical marker based on selected time?
    //    - are vertical ranges even possible?
    //
    // A scalar _literally_ cannot be timeless: we wouldn't even have an x value to work with!
    //
    // - what does the timequery look like?
    //   we're always timed and always sticky,
    // - what happens when the scalar changes color?
    // - what happens when the scalar changes label?
    //
    // what about stuff that has nothing to do with points, e.g. the kind of plot, or whether
    // we want a reference hline/vline to appear?
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

    // TODO: split on colors?

    Plot::new("plot") // TODO
        .legend(Legend::default())
        .show(ui, |plot_ui| {
            dbg!(scene.lines.len());
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
