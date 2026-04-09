use egui::NumExt as _;
use egui::ahash::HashMap;
use re_log_types::{EntityPath, EntityPathHash};
use re_sdk_types::blueprint::archetypes::{PlotBackground, PlotLegend};
use re_sdk_types::blueprint::components::{Corner2D, Enabled};
use re_sdk_types::components::{Color, Visible};
use re_sdk_types::datatypes::TensorBuffer;
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, IconText, MouseButtonText, icons, list_item};
use re_view::controls::SELECTION_RECT_ZOOM_BUTTON;
use re_view::view_property_ui;
use re_viewer_context::{
    IdentifiedViewSystem as _, IndicatedEntities, PerVisualizerType, RecommendedVisualizers,
    ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewId, ViewQuery, ViewState,
    ViewStateExt as _, ViewSystemExecutionError, ViewSystemIdentifier, ViewerContext,
    VisualizableReason, suggest_view_for_each_entity,
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
        _entity_path: &EntityPath,
        visualizers_with_reason: &[(ViewSystemIdentifier, &VisualizableReason)],
        _indicated_entities_per_visualizer: &PerVisualizerType<&IndicatedEntities>,
    ) -> RecommendedVisualizers {
        // Default implementation would not suggest the BarChart visualizer for tensors and 1D images,
        // since they're not indicated with a BarChart indicator.
        // (and as of writing, something needs to be both visualizable and indicated to be shown in a visualizer)

        // Keeping this implementation simple: We know there's only a single visualizer here.
        if visualizers_with_reason
            .iter()
            .any(|(viz, _)| *viz == BarChartVisualizerSystem::identifier())
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
        use egui_plot::{Bar, BarChart, Plot};

        let state = state.downcast_mut::<()>()?;

        let blueprint_db = ctx.blueprint_db();
        let view_id = query.view_id;

        let charts = system_output
            .visualizer_data::<std::collections::BTreeMap<EntityPath, BarChartData>>(
                BarChartVisualizerSystem::identifier(),
            )?;

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

        let legend_id = egui::Id::new(query.view_id).with("plot_legend");
        let legend_hovered = ui
            .ctx()
            .read_response(re_plot::legend::legend_frame_id(legend_id))
            .is_some_and(|r| r.hovered());

        ui.scope(|ui| {
            let background_color = background_color.into();
            ui.style_mut().visuals.extreme_bg_color = background_color;
            let tokens = re_ui::design_tokens_of_visuals(ui.visuals());
            let plot = Plot::new("bar_chart_plot")
                .show_grid(**show_grid)
                .grid_color(tokens.plot_grid_color)
                .grid_fade(tokens.plot_grid_fade)
                .clamp_grid(true)
                .allow_scroll(!legend_hovered);

            // Legend is rendered separately after plot.show() using our LegendWidget.

            // Suppress egui_plot's built-in tooltip so we can render our own.
            let plot = plot.show_x(false).show_y(false);

            let mut plot_item_id_to_entity_path = HashMap::default();

            let mut resolved_bars: Vec<ResolvedBarData> = Vec::new();

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

                let egui_color: egui::Color32 = color.0.into();
                let bars: Vec<(f64, f64, f64)> = arg
                    .iter()
                    .zip(widths.iter())
                    .zip(data.iter())
                    .map(|((index, width), value)| {
                        let center_x = index + (0.5 * *width as f64);
                        (center_x, *width as f64, *value)
                    })
                    .collect();

                plot_item_id_to_entity_path
                    .insert(egui::Id::new(ent_path.hash()), ent_path.clone());

                resolved_bars.push(ResolvedBarData {
                    entity_path: ent_path.clone(),
                    bars,
                    color: egui_color,
                });
            }

            // Load previous frame's transform for hover detection before rendering.
            let plot_id = ui.make_persistent_id(egui::Id::new("bar_chart_plot"));
            let hovered_bar = egui_plot::PlotMemory::load(ui.ctx(), plot_id)
                .map(|mem| mem.transform())
                // Don't hover plot items when hovering legend
                .filter(|_| !legend_hovered)
                .and_then(|prev_transform| find_nearest_bar(ui, &prev_transform, &resolved_bars));

            let egui_plot::PlotResponse {
                response,
                hovered_plot_item: _,
                ..
            } = plot.show(ui, |plot_ui| {
                for resolved in &resolved_bars {
                    let egui_color = resolved.color;
                    let base_fill = if background_color.intensity() < 0.5 {
                        egui_color.gamma_multiply(0.75).additive()
                    } else {
                        egui_color.gamma_multiply(0.75)
                    };
                    let base_stroke_color = base_fill.linear_multiply(0.5);

                    let chart = BarChart::new(
                        "bar_chart",
                        resolved
                            .bars
                            .iter()
                            .enumerate()
                            .map(|(i, (center_x, width, value))| {
                                let is_hovered =
                                    hovered_bar == Some((resolved.entity_path.hash(), i));

                                let stroke = egui::Stroke::new(1.0, base_stroke_color);

                                let (fill, stroke) = if is_hovered {
                                    highlighted_color(stroke, base_fill)
                                } else {
                                    (base_fill, stroke)
                                };

                                Bar::new(*center_x, *value)
                                    .width(*width)
                                    .name(format!("{} #{i}", resolved.entity_path))
                                    .fill(fill)
                                    .stroke(stroke)
                            })
                            .collect(),
                    )
                    .name(resolved.entity_path.to_string())
                    .color(egui_color)
                    .id(egui::Id::new(resolved.entity_path.hash()));

                    plot_ui.bar_chart(chart);
                }
            });

            // Show tooltip and map to selection item for the hovered bar.
            if let Some((entity_hash, bar_idx)) = hovered_bar {
                if let Some(resolved) = resolved_bars
                    .iter()
                    .find(|r| r.entity_path.hash() == entity_hash)
                {
                    let (_center_x, _width, value) = resolved.bars[bar_idx];
                    let entity_path = &resolved.entity_path;

                    re_plot::tooltip::show_plot_tooltip(
                        ui,
                        &response,
                        egui::Id::new(entity_hash).with(bar_idx),
                        &format!("#{bar_idx}"),
                        &entity_path.to_string(),
                        &re_format::format_f64(value),
                        resolved.color,
                    );

                    if let Some(ep) = plot_item_id_to_entity_path.get(&egui::Id::new(entity_hash)) {
                        ctx.viewer_ctx.handle_select_hover_drag_interactions(
                            &response,
                            re_viewer_context::Item::DataResult(
                                re_viewer_context::DataResultInteractionAddress::from_entity_path(
                                    query.view_id,
                                    ep.clone(),
                                ),
                            ),
                            false,
                        );
                    }
                }
            } else if response.hovered() {
                ctx.viewer_ctx.handle_select_hover_drag_interactions(
                    &response,
                    re_viewer_context::Item::View(query.view_id),
                    false,
                );
            }

            // Render our legend overlay.
            if *legend_visible.0 {
                let legend_widget =
                    re_plot::legend::LegendWidget::new(re_plot::legend::LegendConfig {
                        position: legend_corner.into(),
                        id: legend_id,
                    });
                let plot_rect = response.rect;
                let mut legend_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(plot_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );

                let tree = &ctx.query_result.tree;
                let legend_output = legend_widget.show_entries(
                    &mut legend_ui,
                    tree.iter_data_results()
                        .filter(|dr| !dr.tree_prefix_only && charts.contains_key(&dr.entity_path))
                        .map(|dr| {
                            let id = egui::Id::new(dr.entity_path.hash());
                            let color = charts
                                .get(&dr.entity_path)
                                .map(|cd| egui::Color32::from(cd.color.0))
                                .unwrap_or(egui::Color32::GRAY);
                            re_plot::legend::LegendEntry {
                                id,
                                label: dr.entity_path.to_string(),
                                color,
                                visible: dr.is_visible(),
                                hovered: false,
                            }
                        }),
                );

                // Persist visibility changes from legend clicks.
                for dr in tree.iter_data_results() {
                    if dr.tree_prefix_only {
                        continue;
                    }
                    let id = egui::Id::new(dr.entity_path.hash());
                    let new_visible = !legend_output.hidden_ids.contains(&id);
                    if dr.is_visible() != new_visible {
                        dr.save_visible(ctx.viewer_ctx, tree, new_visible);
                    }
                }
            }
        });

        Ok(())
    }
}

struct ResolvedBarData {
    entity_path: EntityPath,

    /// `(bar_center_x, bar_width, value)`
    bars: Vec<(f64, f64, f64)>,
    color: egui::Color32,
}

/// Compute a highlighted color for a bar: brighten the fill and darken the stroke.
///
/// Matches the behavior of `egui_plot`.
fn highlighted_color(stroke: egui::Stroke, fill: egui::Color32) -> (egui::Color32, egui::Stroke) {
    let mut fill_rgba = egui::Rgba::from(fill);
    if fill_rgba.is_additive() {
        fill_rgba = 1.3 * fill_rgba;
    } else {
        let fill_alpha = (2.0 * fill_rgba.a()).at_most(1.0);
        fill_rgba = fill_rgba.to_opaque().multiply(fill_alpha);
    }

    let stroke_rgba = egui::Rgba::from(stroke.color) * 0.5;
    let highlight_stroke = egui::Stroke::new(stroke.width, egui::Color32::from(stroke_rgba));

    (fill_rgba.into(), highlight_stroke)
}

const TOOLTIP_INTERACT_RADIUS: f32 = 16.0;

/// Find the nearest bar to the cursor. Returns `(entity_path_hash, bar_index)`.
fn find_nearest_bar(
    ui: &egui::Ui,
    transform: &egui_plot::PlotTransform,
    resolved_bars: &[ResolvedBarData],
) -> Option<(EntityPathHash, usize)> {
    let hover_pos = ui.input(|i| i.pointer.hover_pos())?;

    // Only detect hovers within the plot rect.
    if !transform.frame().contains(hover_pos) {
        return None;
    }

    let mut closest: Option<(f32, EntityPathHash, usize)> = None;

    for resolved in resolved_bars {
        let entity_hash = resolved.entity_path.hash();
        for (bar_idx, (center_x, width, value)) in resolved.bars.iter().enumerate() {
            // Check proximity to bar top.
            let bar_screen_pos =
                transform.position_from_point(&egui_plot::PlotPoint::new(*center_x, *value));
            let dist = hover_pos.distance(bar_screen_pos);
            if dist <= TOOLTIP_INTERACT_RADIUS
                && closest.is_none_or(|(best_dist, _, _)| dist < best_dist)
            {
                closest = Some((dist, entity_hash, bar_idx));
            }

            // Also check if cursor is inside the bar rectangle.
            let bar_left = transform
                .position_from_point(&egui_plot::PlotPoint::new(center_x - width / 2.0, 0.0));
            let bar_right = transform
                .position_from_point(&egui_plot::PlotPoint::new(center_x + width / 2.0, 0.0));
            if hover_pos.x >= bar_left.x && hover_pos.x <= bar_right.x {
                let bar_top =
                    transform.position_from_point(&egui_plot::PlotPoint::new(*center_x, *value));
                let bar_bottom =
                    transform.position_from_point(&egui_plot::PlotPoint::new(*center_x, 0.0));
                let (top_y, bottom_y) = if bar_top.y < bar_bottom.y {
                    (bar_top.y, bar_bottom.y)
                } else {
                    (bar_bottom.y, bar_top.y)
                };
                if hover_pos.y >= top_y && hover_pos.y <= bottom_y {
                    // Cursor is inside the bar — this is the best match.
                    return Some((entity_hash, bar_idx));
                }
            }
        }
    }

    closest.map(|(_, hash, idx)| (hash, idx))
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| BarChartView.help(ctx));
}
