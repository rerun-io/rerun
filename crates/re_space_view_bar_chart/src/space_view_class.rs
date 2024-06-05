use egui::{ahash::HashMap, util::hash};
use re_entity_db::{EditableAutoValue, EntityProperties, LegendCorner};
use re_log_types::EntityPath;
use re_space_view::{controls, suggest_space_view_for_each_entity};
use re_types::{datatypes::TensorBuffer, SpaceViewClassIdentifier};
use re_viewer_context::{
    auto_color, IdentifiedViewSystem as _, IndicatedEntities, PerVisualizer, SpaceViewClass,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewSystemExecutionError,
    ViewQuery, ViewerContext, VisualizableEntities,
};

use super::visualizer_system::BarChartVisualizerSystem;

#[derive(Default)]
pub struct BarChartSpaceView;

use re_types::View;
type ViewType = re_types::blueprint::views::BarChartView;

impl SpaceViewClass for BarChartSpaceView {
    fn identifier() -> SpaceViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Bar chart"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_HISTOGRAM
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<()>::default()
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

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        None
    }

    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizer<VisualizableEntities>,
        _indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> re_viewer_context::SmallVisualizerSet {
        // Default implementation would not suggest the BarChart visualizer for tensors and 1D images,
        // since they're not indicated with a BarChart indicator.
        // (and as of writing, something needs to be both visualizable and indicated to be shown in a visualizer)

        // Keeping this implementation simple: We know there's only a single visualizer here.
        if visualizable_entities_per_visualizer
            .get(&BarChartVisualizerSystem::identifier())
            .map_or(false, |entities| entities.contains(entity_path))
        {
            std::iter::once(BarChartVisualizerSystem::identifier()).collect()
        } else {
            Default::default()
        }
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();
        suggest_space_view_for_each_entity::<BarChartVisualizerSystem>(ctx, self)
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        root_entity_properties: &mut EntityProperties,
    ) -> Result<(), SpaceViewSystemExecutionError> {
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

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn SpaceViewState,
        root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
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
                plot = plot.legend(
                    Legend::default().position(to_egui_plot_corner(
                        root_entity_properties
                            .legend_location
                            .unwrap_or(LegendCorner::RightTop),
                    )),
                );
            }

            let mut plot_item_id_to_entity_path = HashMap::default();

            let egui_plot::PlotResponse {
                response,
                hovered_plot_item,
                ..
            } = plot.show(ui, |plot_ui| {
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
                        TensorBuffer::Yuy2(_) => {
                            re_log::warn_once!(
                                "trying to display YUY2 data as a bar chart ({:?})",
                                ent_path
                            );
                            continue;
                        }
                    };

                    let id = egui::Id::new(ent_path.hash());
                    plot_item_id_to_entity_path.insert(id, ent_path.clone());
                    let chart = chart.id(id);

                    plot_ui.bar_chart(chart);
                }
            });

            // Interact with the plot items.
            if let Some(entity_path) = hovered_plot_item
                .and_then(|hovered_plot_item| plot_item_id_to_entity_path.get(&hovered_plot_item))
            {
                ctx.select_hovered_on_click(
                    &response,
                    re_viewer_context::Item::DataResult(
                        query.space_view_id,
                        entity_path.clone().into(),
                    ),
                );
            }
        });

        Ok(())
    }
}

fn to_egui_plot_corner(value: LegendCorner) -> egui_plot::Corner {
    match value {
        LegendCorner::LeftTop => egui_plot::Corner::LeftTop,
        LegendCorner::RightTop => egui_plot::Corner::RightTop,
        LegendCorner::LeftBottom => egui_plot::Corner::LeftBottom,
        LegendCorner::RightBottom => egui_plot::Corner::RightBottom,
    }
}

re_viewer_context::impl_component_fallback_provider!(BarChartSpaceView => []);
