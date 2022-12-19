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

    let stroke = egui::Stroke::new(1.0, ui.visuals().extreme_bg_color); // Same as plot background fill

    Plot::new("bar_chart_plot")
        .legend(Legend::default())
        .show(ui, |plot_ui| {
            for (instance_id, bar_chart_values) in &scene.charts {
                let [r, g, b, a] = auto_color(hash(instance_id) as _);
                let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                let fill = color.gamma_multiply(0.75).additive(); // make sure overlapping bars are obvious

                let chart = BarChart::new(
                    bar_chart_values
                        .values
                        .iter()
                        .enumerate()
                        .map(|(i, value)| {
                            Bar::new(i as f64 + 0.5, *value as f64)
                                .width(1.0)
                                .name(format!("{instance_id} #{i}"))
                                .fill(fill)
                                .stroke(stroke)
                        })
                        .collect(),
                )
                .name(instance_id.to_string())
                .color(color);

                plot_ui.bar_chart(chart);
            }
        })
        .response
}
