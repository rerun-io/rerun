use egui::util::hash;
use re_components::TensorData;
use re_log_types::EntityPath;
use re_space_view::{controls, EmptySpaceViewState};
use re_viewer_context::{
    auto_color, SpaceViewClass, SpaceViewClassName, SpaceViewId, TypedScene, ViewerContext,
};

use super::scene_part::SceneBarChart;

#[derive(Default)]
pub struct BarChartSpaceView;

impl SpaceViewClass for BarChartSpaceView {
    type State = EmptySpaceViewState;
    type Context = re_space_view::EmptySceneContext;
    type SceneParts = SceneBarChart;
    type ScenePartData = ();

    fn name(&self) -> SpaceViewClassName {
        "Bar Chart".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_HISTOGRAM
    }

    fn help_text(&self, re_ui: &re_ui::ReUi, _state: &Self::State) -> egui::WidgetText {
        let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

        layout.add("Pan by dragging, or scroll (+ ");
        layout.add(controls::HORIZONTAL_SCROLL_MODIFIER);
        layout.add(" for horizontal).\n");

        layout.add("Zoom with pinch gesture or scroll + ");
        layout.add(controls::ZOOM_SCROLL_MODIFIER);
        layout.add(".\n");

        layout.add("Drag ");
        layout.add(controls::SELECTION_RECT_ZOOM_BUTTON);
        layout.add(" to zoom in/out using a selection.\n\n");

        layout.add_button_text("double-click");
        layout.add(" to reset the view.");

        layout.layout_job.into()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn selection_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
    }

    fn ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        scene: &mut TypedScene<Self>,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
        use egui::plot::{Bar, BarChart, Legend, Plot};

        ui.scope(|ui| {
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

                    for (ent_path, tensor) in &scene.parts.charts {
                        let chart = match &tensor.data {
                            TensorData::U8(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::U16(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::U32(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::U64(data) => {
                                create_bar_chart(ent_path, data.iter().copied().map(|v| v as f64))
                            }
                            TensorData::I8(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::I16(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::I32(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::I64(data) => {
                                create_bar_chart(ent_path, data.iter().copied().map(|v| v as f64))
                            }
                            TensorData::F16(data) => {
                                create_bar_chart(ent_path, data.iter().map(|f| f.to_f32()))
                            }
                            TensorData::F32(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::F64(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorData::JPEG(_) => {
                                re_log::warn_once!(
                                    "trying to display JPEG data as a bar chart ({:?})",
                                    ent_path
                                );
                                continue;
                            }
                        };

                        plot_ui.bar_chart(chart);
                    }
                });
        });
    }
}
