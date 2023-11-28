use egui::util::hash;
use re_data_store::EntityProperties;
use re_log_types::EntityPath;
use re_space_view::controls;
use re_types::datatypes::TensorBuffer;
use re_viewer_context::{
    auto_color, SpaceViewClass, SpaceViewClassName, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewPartCollection, ViewQuery,
    ViewerContext,
};

use super::view_part_system::BarChartViewPartSystem;

#[derive(Default)]
pub struct BarChartSpaceView;

impl SpaceViewClass for BarChartSpaceView {
    type State = ();

    fn name(&self) -> SpaceViewClassName {
        "Bar Chart".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_HISTOGRAM
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

        layout.add("Pan by dragging, or scroll (+ ");
        layout.add(controls::HORIZONTAL_SCROLL_MODIFIER);
        layout.add(" for horizontal).\n");

        layout.add("Zoom with pinch gesture or scroll + ");
        layout.add(controls::ZOOM_SCROLL_MODIFIER);
        layout.add(".\n");

        layout.add("Scroll + ");
        layout.add(controls::ASPECT_SCROLL_MODIFIER);
        layout.add(" to change the aspect ratio.\n");

        layout.add("Drag ");
        layout.add(controls::SELECTION_RECT_ZOOM_BUTTON);
        layout.add(" to zoom in/out using a selection.\n\n");

        layout.add_button_text("double-click");
        layout.add(" to reset the view.");

        layout.layout_job.into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_part_system::<BarChartViewPartSystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
    }

    fn ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        _view_ctx: &ViewContextCollection,
        parts: &ViewPartCollection,
        _query: &ViewQuery<'_>,
        _draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        use egui_plot::{Bar, BarChart, Legend, Plot};

        let charts = &parts.get::<BarChartViewPartSystem>()?.charts;

        let zoom_both_axis = !ui.input(|i| i.modifiers.contains(controls::ASPECT_SCROLL_MODIFIER));

        ui.scope(|ui| {
            Plot::new("bar_chart_plot")
                .legend(Legend::default())
                .clamp_grid(true)
                .allow_zoom(egui_plot::AxisBools {
                    x: true,
                    y: zoom_both_axis,
                })
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

                    for (ent_path, tensor) in charts {
                        let chart = match &tensor.buffer {
                            TensorBuffer::U8(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::U16(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::U32(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::U64(data) => {
                                create_bar_chart(ent_path, data.iter().copied().map(|v| v as f64))
                            }
                            TensorBuffer::I8(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::I16(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::I32(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::I64(data) => {
                                create_bar_chart(ent_path, data.iter().copied().map(|v| v as f64))
                            }
                            TensorBuffer::F16(data) => {
                                create_bar_chart(ent_path, data.iter().map(|f| f.to_f32()))
                            }
                            TensorBuffer::F32(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::F64(data) => {
                                create_bar_chart(ent_path, data.iter().copied())
                            }
                            TensorBuffer::Jpeg(_) => {
                                re_log::warn_once!(
                                    "trying to display JPEG data as a bar chart ({:?})",
                                    ent_path
                                );
                                continue;
                            }
                            TensorBuffer::Nv12(_) => {
                                re_log::warn_once!(
                                    "trying to display NV12 data as a bar chart ({:?})",
                                    ent_path
                                );
                                continue;
                            }
                        };

                        plot_ui.bar_chart(chart);
                    }
                });
        });

        Ok(())
    }
}
