use egui::ahash::HashMap;
use egui_plot::ColorConflictHandling;
use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::{PlotBackground, PlotLegend};
use re_sdk_types::blueprint::components::{Corner2D, Enabled};
use re_sdk_types::components::{Color, Visible};
use re_sdk_types::datatypes::TensorBuffer;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, IconText, MouseButtonText, icons, list_item};
use re_view::controls::SELECTION_RECT_ZOOM_BUTTON;
use re_view::view_property_ui;
use re_viewer_context::{
    IdentifiedViewSystem as _, IndicatedEntities, PerVisualizerType, PerVisualizerTypeInViewClass,
    RecommendedVisualizers, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewId,
    ViewQuery, ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
    VisualizableEntities, suggest_view_for_each_entity,
};
use re_viewport_blueprint::ViewProperty;

use super::visualizer_system::{BarChartData, BarChartVisualizerSystem};

#[derive(Default)]
pub struct BarChartView;

type ViewType = re_sdk_types::blueprint::views::BarChartView;

impl ViewClass for BarChartView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Bar chart"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_HISTOGRAM
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<()>::default()
    }

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        let egui::InputOptions {
            zoom_modifier,
            horizontal_scroll_modifier,
            vertical_scroll_modifier,
            ..
        } = egui::InputOptions::default(); // This is OK, since we don't allow the user to change these modifiers.

        Help::new("Bar chart view")
            .docs_link("https://rerun.io/docs/reference/types/views/bar_chart_view")
            .control("Pan", (icons::LEFT_MOUSE_CLICK, "+", "drag"))
            .control(
                "Horizontal pan",
                IconText::from_modifiers_and(os, horizontal_scroll_modifier, icons::SCROLL),
            )
            .control(
                "Zoom",
                IconText::from_modifiers_and(os, zoom_modifier, icons::SCROLL),
            )
            .control(
                "Zoom X-axis",
                IconText::from_modifiers_and(
                    os,
                    zoom_modifier | horizontal_scroll_modifier,
                    icons::SCROLL,
                ),
            )
            .control(
                "Zoom Y-axis",
                IconText::from_modifiers_and(
                    os,
                    zoom_modifier | vertical_scroll_modifier,
                    icons::SCROLL,
                ),
            )
            .control(
                "Zoom to selection",
                (MouseButtonText(SELECTION_RECT_ZOOM_BUTTON), "+", "drag"),
            )
            .control("Reset view", ("double", icons::LEFT_MOUSE_CLICK))
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_visualizer::<BarChartVisualizerSystem>()?;

        system_registry.register_fallback_provider::<Corner2D>(
            PlotLegend::descriptor_corner().component,
            |_| Corner2D::RightTop,
        );

        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        None
    }

    fn recommended_visualizers_for_entity(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizerTypeInViewClass<VisualizableEntities>,
        _indicated_entities_per_visualizer: &PerVisualizerType<IndicatedEntities>,
    ) -> RecommendedVisualizers {
        // Default implementation would not suggest the BarChart visualizer for tensors and 1D images,
        // since they're not indicated with a BarChart indicator.
        // (and as of writing, something needs to be both visualizable and indicated to be shown in a visualizer)

        // Keeping this implementation simple: We know there's only a single visualizer here.
        if visualizable_entities_per_visualizer
            .get(&BarChartVisualizerSystem::identifier())
            .is_some_and(|entities| entities.contains_key(entity_path))
        {
            RecommendedVisualizers::default(BarChartVisualizerSystem::identifier())
        } else {
            RecommendedVisualizers::empty()
        }
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();
        suggest_view_for_each_entity::<BarChartVisualizerSystem>(ctx, include_entity)
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        list_item::list_item_scope(ui, "bar_char_selection_ui", |ui| {
            let ctx = self.view_context(ctx, view_id, state, space_origin);
            view_property_ui::<PlotBackground>(&ctx, ui);
            view_property_ui::<PlotLegend>(&ctx, ui);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        use egui_plot::{Bar, BarChart, Legend, Plot};

        let state = state.downcast_mut::<()>()?;

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.view_id;

        let charts = &system_output
            .view_systems
            .get::<BarChartVisualizerSystem>()?
            .charts;

        let ctx = self.view_context(ctx, view_id, state, query.space_origin);
        let background = ViewProperty::from_archetype::<PlotBackground>(
            blueprint_db,
            ctx.blueprint_query(),
            view_id,
        );
        let background_color = background
            .component_or_fallback::<Color>(&ctx, PlotBackground::descriptor_color().component)?;
        let show_grid = background.component_or_fallback::<Enabled>(
            &ctx,
            PlotBackground::descriptor_show_grid().component,
        )?;

        let plot_legend = ViewProperty::from_archetype::<PlotLegend>(
            blueprint_db,
            ctx.blueprint_query(),
            view_id,
        );
        let legend_visible: Visible =
            plot_legend.component_or_fallback(&ctx, PlotLegend::descriptor_visible().component)?;
        let legend_corner: Corner2D =
            plot_legend.component_or_fallback(&ctx, PlotLegend::descriptor_corner().component)?;

        ui.scope(|ui| {
            let background_color = background_color.into();
            ui.style_mut().visuals.extreme_bg_color = background_color;
            let mut plot = Plot::new("bar_chart_plot")
                .show_grid(**show_grid)
                .clamp_grid(true);

            if *legend_visible.0 {
                plot = plot.legend(
                    Legend::default()
                        .position(legend_corner.into())
                        .color_conflict_handling(ColorConflictHandling::PickFirst),
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
                    indexes: impl Iterator<Item = f64>,
                    widths: impl Iterator<Item = f32>,
                    values: impl Iterator<Item = N>,
                    color: &re_sdk_types::components::Color,
                    background_color: egui::Color32,
                ) -> BarChart {
                    let color: egui::Color32 = color.0.into();
                    let fill = if background_color.intensity() < 0.5 {
                        color.gamma_multiply(0.75).additive() // make sure overlapping bars are obvious for darker background colors.
                    } else {
                        color.gamma_multiply(0.75)
                    };
                    let stroke_color = fill.linear_multiply(0.5);
                    BarChart::new(
                        "bar_chart",
                        values
                            .zip(indexes)
                            .zip(widths)
                            .enumerate()
                            .map(|(i, ((value, index), width))| {
                                Bar::new(index + (0.5 * width as f64), value.into())
                                    .width(width as f64)
                                    .name(format!("{ent_path} #{i}"))
                                    .fill(fill)
                                    .stroke((1.0, stroke_color))
                            })
                            .collect(),
                    )
                    .name(ent_path.to_string())
                    .color(color)
                }

                for (
                    ent_path,
                    BarChartData {
                        abscissa,
                        values: tensor,
                        color,
                        widths,
                    },
                ) in charts
                {
                    let arg: ::arrow::buffer::ScalarBuffer<f64> = match &abscissa.buffer {
                        TensorBuffer::U8(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::U16(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::U32(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::U64(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I8(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I16(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I32(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I64(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::F16(data) => data.iter().map(|v| f64::from(*v)).collect(),
                        TensorBuffer::F32(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::F64(data) => data.iter().copied().collect(),
                    };

                    let data: ::arrow::buffer::ScalarBuffer<f64> = match &tensor.buffer {
                        TensorBuffer::U8(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::U16(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::U32(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::U64(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I8(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I16(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I32(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::I64(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::F16(data) => data.iter().map(|v| f64::from(*v)).collect(),
                        TensorBuffer::F32(data) => data.iter().map(|v| *v as f64).collect(),
                        TensorBuffer::F64(data) => data.iter().copied().collect(),
                    };
                    let chart = create_bar_chart(
                        ent_path,
                        arg.iter().copied(),
                        widths.iter().copied(),
                        data.iter().copied(),
                        color,
                        background_color,
                    );

                    let id = egui::Id::new(ent_path.hash());
                    plot_item_id_to_entity_path.insert(id, ent_path.clone());
                    let chart = chart.id(id);

                    plot_ui.bar_chart(chart);
                }
            });

            // Interact with the plot items.
            let hovered_data_result = hovered_plot_item
                .and_then(|hovered_plot_item| plot_item_id_to_entity_path.get(&hovered_plot_item))
                .map(|entity_path| {
                    re_viewer_context::Item::DataResult(query.view_id, entity_path.clone().into())
                })
                .or_else(|| {
                    if response.hovered() {
                        Some(re_viewer_context::Item::View(query.view_id))
                    } else {
                        None
                    }
                });
            if let Some(hovered) = hovered_data_result {
                ctx.viewer_ctx
                    .handle_select_hover_drag_interactions(&response, hovered, false);
            }
        });

        Ok(())
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| BarChartView.help(ctx));
}
