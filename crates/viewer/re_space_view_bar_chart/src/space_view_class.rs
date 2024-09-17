use egui::ahash::HashMap;
use re_log_types::EntityPath;
use re_space_view::controls::{
    ASPECT_SCROLL_MODIFIER, HORIZONTAL_SCROLL_MODIFIER, SELECTION_RECT_ZOOM_BUTTON,
    ZOOM_SCROLL_MODIFIER,
};
use re_space_view::{controls, suggest_space_view_for_each_entity, view_property_ui};
use re_types::blueprint::archetypes::PlotLegend;
use re_types::blueprint::components::{Corner2D, Visible};
use re_types::View;
use re_types::{datatypes::TensorBuffer, SpaceViewClassIdentifier};
use re_ui::{list_item, ModifiersMarkdown, MouseButtonMarkdown};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem as _, IndicatedEntities, PerVisualizer,
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewStateExt,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewQuery, ViewerContext,
    VisualizableEntities,
};
use re_viewport_blueprint::ViewProperty;

use super::visualizer_system::BarChartVisualizerSystem;

#[derive(Default)]
pub struct BarChartSpaceView;

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

    fn help_markdown(&self, egui_ctx: &egui::Context) -> String {
        format!(
            "# Bar chart view

Display a 1D tensor as a bar chart.

## Navigation controls

- Pan by dragging, or scroll (+{horizontal_scroll_modifier} for horizontal).
- Zoom with pinch gesture or scroll + {zoom_scroll_modifier}.
- Scroll + {aspect_scroll_modifier} to zoom only the temporal axis while holding the y-range fixed.
- Drag with the {selection_rect_zoom_button} to zoom in/out using a selection.
- Double-click to reset the view.",
            horizontal_scroll_modifier = ModifiersMarkdown(HORIZONTAL_SCROLL_MODIFIER, egui_ctx),
            zoom_scroll_modifier = ModifiersMarkdown(ZOOM_SCROLL_MODIFIER, egui_ctx),
            aspect_scroll_modifier = ModifiersMarkdown(ASPECT_SCROLL_MODIFIER, egui_ctx),
            selection_rect_zoom_button = MouseButtonMarkdown(SELECTION_RECT_ZOOM_BUTTON),
        )
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
        _applicable_entities_per_visualizer: &PerVisualizer<ApplicableEntities>,
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
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        list_item::list_item_scope(ui, "time_series_selection_ui", |ui| {
            view_property_ui::<PlotLegend>(ctx, ui, space_view_id, self, state);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,

        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        use egui_plot::{Bar, BarChart, Legend, Plot};

        let state = state.downcast_mut::<()>()?;

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.space_view_id;

        let charts = &system_output
            .view_systems
            .get::<BarChartVisualizerSystem>()?
            .charts;

        let zoom_both_axis = !ui.input(|i| i.modifiers.contains(controls::ASPECT_SCROLL_MODIFIER));

        let plot_legend =
            ViewProperty::from_archetype::<PlotLegend>(blueprint_db, ctx.blueprint_query, view_id);
        let legend_visible = plot_legend.component_or_fallback::<Visible>(ctx, self, state)?;
        let legend_corner = plot_legend.component_or_fallback::<Corner2D>(ctx, self, state)?;

        ui.scope(|ui| {
            let mut plot = Plot::new("bar_chart_plot")
                .clamp_grid(true)
                .allow_zoom([true, zoom_both_axis]);

            if *legend_visible.0 {
                plot = plot.legend(Legend::default().position(legend_corner.into()));
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
                    color: &re_types::components::Color,
                ) -> BarChart {
                    let color: egui::Color32 = color.0.into();
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

impl TypedComponentFallbackProvider<Corner2D> for BarChartSpaceView {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Corner2D {
        // Explicitly pick RightCorner2D::RightTop, we don't want to make this dependent on the (arbitrary)
        // default of Corner2D
        Corner2D::RightTop
    }
}

re_viewer_context::impl_component_fallback_provider!(BarChartSpaceView => [Corner2D]);
