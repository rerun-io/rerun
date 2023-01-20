// ---

use egui::util::hash;

use crate::{misc::ViewerContext, ui::annotations::auto_color};

use super::SceneBarChart;

// ---

pub(crate) const HELP_TEXT: &str = "\
    Pan by dragging, or scroll (+ shift = horizontal).\n\
    Box zooming: Right click to zoom in and zoom out using a selection.\n\
    Reset view with double-click.";

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct BarChartState;

pub(crate) fn view_bar_chart(
    _ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    _state: &mut BarChartState,
    scene: &SceneBarChart,
) -> egui::Response {
    use egui::plot::{Bar, BarChart, Legend, Plot};

    Plot::new("bar_chart_plot")
        .legend(Legend::default())
        .clamp_grid(true)
        .show(ui, |plot_ui| {
            for (instance_id, bar_chart_values) in &scene.charts {
                let color = auto_color(hash(instance_id) as _);
                let fill = color.gamma_multiply(0.75).additive(); // make sure overlapping bars are obvious

                plot_ui.bar_chart(
                    BarChart::new(
                        bar_chart_values
                            .values
                            .iter()
                            .enumerate()
                            .map(|(i, value)| {
                                Bar::new(i as f64 + 0.5, *value)
                                    .width(0.95)
                                    .name(format!("{instance_id} #{i}"))
                                    .fill(fill)
                                    .stroke(egui::Stroke::NONE)
                            })
                            .collect(),
                    )
                    .name(instance_id.to_string())
                    .color(color),
                );
            }
        })
        .response
}
