use egui::util::hash;
use re_data_store::{EditableAutoValue, EntityProperties, LegendCorner};
use re_log_types::EntityPath;
use re_space_view::controls;
use re_types::datatypes::TensorBuffer;
use re_viewer_context::{
    auto_color, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
};

use super::visualizer_system::BarChartVisualizerSystem;

#[derive(Default)]
pub struct BarChartSpaceView;

impl SpaceViewClass for BarChartSpaceView {
    type State = ();

    const IDENTIFIER: &'static str = "Bar Chart";
    const DISPLAY_NAME: &'static str = "Bar Chart";

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
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<BarChartVisualizerSystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        root_entity_properties: &mut EntityProperties,
    ) {
        ctx.re_ui
            .selection_grid(ui, "bar_chart_selection_ui")
            .show(ui, |ui| {
                ctx.re_ui.grid_left_hand_label(ui, "Legend");

                ui.vertical(|ui| {
                    let mut selected = *root_entity_properties.show_legend.get();
                    if ctx.re_ui.checkbox(ui, &mut selected, "Visible").changed() {
                        root_entity_properties.show_legend =
                            EditableAutoValue::UserEdited(selected);
                    }

                    let mut corner = root_entity_properties
                        .legend_location
                        .unwrap_or(LegendCorner::RightTop);

                    egui::ComboBox::from_id_source("legend_corner")
                        .selected_text(corner.to_string())
                        .show_ui(ui, |ui| {
                            ui.style_mut().wrap = Some(false);
                            ui.set_min_width(64.0);

                            ui.selectable_value(
                                &mut corner,
                                LegendCorner::LeftTop,
                                LegendCorner::LeftTop.to_string(),
                            );
                            ui.selectable_value(
                                &mut corner,
                                LegendCorner::RightTop,
                                LegendCorner::RightTop.to_string(),
                            );
                            ui.selectable_value(
                                &mut corner,
                                LegendCorner::LeftBottom,
                                LegendCorner::LeftBottom.to_string(),
                            );
                            ui.selectable_value(
                                &mut corner,
                                LegendCorner::RightBottom,
                                LegendCorner::RightBottom.to_string(),
                            );
                        });

                    root_entity_properties.legend_location = Some(corner);
                });
                ui.end_row();
            });
    }

    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        root_entity_properties: &EntityProperties,
        _query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        use egui_plot::{Bar, BarChart, Legend, Plot};

        let charts = &system_output
            .view_systems
            .get::<BarChartVisualizerSystem>()?
            .charts;

        let zoom_both_axis = !ui.input(|i| i.modifiers.contains(controls::ASPECT_SCROLL_MODIFIER));

        ui.scope(|ui| {
            let mut plot = Plot::new("bar_chart_plot")
                .clamp_grid(true)
                .allow_zoom([true, zoom_both_axis]);

            if *root_entity_properties.show_legend {
                plot = plot.legend(Legend {
                    position: root_entity_properties
                        .legend_location
                        .unwrap_or(LegendCorner::RightTop)
                        .into(),
                    ..Default::default()
                });
            }

            plot.show(ui, |plot_ui| {
                fn create_bar_chart<N: Into<f64>>(
                    ent_path: &EntityPath,
                    values: impl Iterator<Item = N>,
                    color: &Option<re_types::components::Color>,
                ) -> BarChart {
                    let color =
                        color.map_or_else(|| auto_color(hash(ent_path) as _), |color| color.into());
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

                for (ent_path, (tensor, color)) in charts {
                    let chart = match &tensor.buffer {
                        TensorBuffer::U8(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
                        }
                        TensorBuffer::U16(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
                        }
                        TensorBuffer::U32(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
                        }
                        TensorBuffer::U64(data) => create_bar_chart(
                            ent_path,
                            data.iter().copied().map(|v| v as f64),
                            color,
                        ),
                        TensorBuffer::I8(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
                        }
                        TensorBuffer::I16(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
                        }
                        TensorBuffer::I32(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
                        }
                        TensorBuffer::I64(data) => create_bar_chart(
                            ent_path,
                            data.iter().copied().map(|v| v as f64),
                            color,
                        ),
                        TensorBuffer::F16(data) => {
                            create_bar_chart(ent_path, data.iter().map(|f| f.to_f32()), color)
                        }
                        TensorBuffer::F32(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
                        }
                        TensorBuffer::F64(data) => {
                            create_bar_chart(ent_path, data.iter().copied(), color)
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
