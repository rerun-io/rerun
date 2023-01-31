// ---

use egui::util::hash;

use re_data_store::EntityPath;
use re_log::warn_once;
use re_log_types::component_types::{self, InstanceKey};

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
            fn create_bar_chart<N: Into<f64>>(
                ent_path: &EntityPath,
                instance_key: &InstanceKey,
                values: impl Iterator<Item = N>,
            ) -> BarChart {
                let color = auto_color(hash((ent_path, instance_key)) as _);
                let fill = color.gamma_multiply(0.75).additive(); // make sure overlapping bars are obvious
                BarChart::new(
                    values
                        .enumerate()
                        .map(|(i, value)| {
                            Bar::new(i as f64 + 0.5, value.into())
                                .width(0.95)
                                .name(format!("{ent_path}[#{instance_key}] #{i}"))
                                .fill(fill)
                                .stroke(egui::Stroke::NONE)
                        })
                        .collect(),
                )
                .name(format!("{ent_path}[#{instance_key}]"))
                .color(color)
            }

            for ((ent_path, instance_key), tensor) in &scene.charts {
                let chart = match &tensor.data {
                    component_types::TensorData::U8(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::U16(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::U32(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::U64(data) => create_bar_chart(
                        ent_path,
                        instance_key,
                        data.iter().copied().map(|v| v as f64),
                    ),
                    component_types::TensorData::I8(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::I16(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::I32(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::I64(data) => create_bar_chart(
                        ent_path,
                        instance_key,
                        data.iter().copied().map(|v| v as f64),
                    ),
                    component_types::TensorData::F32(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::F64(data) => {
                        create_bar_chart(ent_path, instance_key, data.iter().copied())
                    }
                    component_types::TensorData::JPEG(_) => {
                        warn_once!(
                            "trying to display JPEG data as a bar chart ({:?})",
                            ent_path
                        );
                        continue;
                    }
                };

                plot_ui.bar_chart(chart);
            }
        })
        .response
}
