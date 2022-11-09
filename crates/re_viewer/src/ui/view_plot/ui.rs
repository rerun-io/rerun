use crate::ViewerContext;
use egui::plot::{Legend, Line, LineStyle, Plot, PlotPoints};
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
    // - plug in Legend/ClassDescr?
    // - position marker
    // - log_scalars doesn't make sense!
    // - does the time thing behave correctly here? what about multi-timeline setups?
    //
    // - document how spaces work for plots

    let lines = scene
        .plots
        .iter()
        .map(|(obj_path, plot)| {
            let points = plot.iter().map(|s| [s.time as _, s.value]).collect();
            let points = PlotPoints::new(points);
            let line = Line::new(points).name(obj_path).width(3.0);
            (obj_path, line)
        })
        .collect::<Vec<_>>();

    Plot::new("plot_view")
        .legend(Legend::default())
        .show(ui, move |plot_ui| {
            for (obj_path, line) in lines {
                plot_ui.line(line)
            }
        })
        .response
}
