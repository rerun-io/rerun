// ---

use egui::util::hash;

use re_data_store::EntityPath;
use re_log::warn_once;
use re_log_types::component_types;
use re_viewer_context::{auto_color, ViewerContext};

use crate::ui::spaceview_controls::{
    HORIZONTAL_SCROLL_MODIFIER, SELECTION_RECT_ZOOM_BUTTON, ZOOM_SCROLL_MODIFIER,
};

use super::SceneBarChart;

// ---

pub fn help_text(re_ui: &re_ui::ReUi) -> egui::WidgetText {
    let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

    layout.add("Pan by dragging, or scroll (+ ");
    layout.add(HORIZONTAL_SCROLL_MODIFIER);
    layout.add(" for horizontal).\n");

    layout.add("Zoom with pinch gesture or scroll + ");
    layout.add(ZOOM_SCROLL_MODIFIER);
    layout.add(".\n");

    layout.add("Drag ");
    layout.add(SELECTION_RECT_ZOOM_BUTTON);
    layout.add(" to zoom in/out using a selection.\n\n");

    layout.add_button_text("double-click");
    layout.add(" to reset the view.");

    layout.layout_job.into()
}

#[derive(Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
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
                values: impl Iterator<Item = N>,
            ) -> BarChart {
                let color = auto_color(hash(ent_path) as _);
                let fill = color.gamma_multiply(0.75).additive(); // make sure overlapping bars are obvious
                BarChart::new(
                    values
                        .enumerate()
                        .map(|(i, value)| {
                            Bar::new(i as f64 + 0.5, value.into())
                                .width(0.95)
                                .name(format!("{ent_path} #{i}"))
                                .fill(fill)
                                .stroke(egui::Stroke::NONE)
                        })
                        .collect(),
                )
                .name(ent_path.to_string())
                .color(color)
            }

            for (ent_path, tensor) in &scene.charts {
                let chart = match &tensor.data {
                    component_types::TensorData::U8(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
                    }
                    component_types::TensorData::U16(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
                    }
                    component_types::TensorData::U32(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
                    }
                    component_types::TensorData::U64(data) => {
                        create_bar_chart(ent_path, data.iter().copied().map(|v| v as f64))
                    }
                    component_types::TensorData::I8(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
                    }
                    component_types::TensorData::I16(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
                    }
                    component_types::TensorData::I32(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
                    }
                    component_types::TensorData::I64(data) => {
                        create_bar_chart(ent_path, data.iter().copied().map(|v| v as f64))
                    }
                    component_types::TensorData::F16(data) => {
                        create_bar_chart(ent_path, data.iter().map(|f| f.to_f32()))
                    }
                    component_types::TensorData::F32(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
                    }
                    component_types::TensorData::F64(data) => {
                        create_bar_chart(ent_path, data.iter().copied())
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
