use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::{
    ForceCenter, ForceCollisionRadius, ForceLink, ForceManyBody, ForcePosition, GraphBackground,
    VisualBounds2D,
};
use re_sdk_types::components::Color;
use re_sdk_types::{ViewClassIdentifier, blueprint};
use re_ui::{self, Help, IconText, MouseButtonText, UiExt as _, icons};
use re_view::controls::DRAG_PAN2D_BUTTON;
use re_view::view_property_ui;
use re_viewer_context::{
    Item, SystemCommand, SystemCommandSender as _, SystemExecutionOutput, ViewClass,
    ViewClassExt as _, ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewSystemRegistrator, ViewerContext, suggest_view_for_each_entity,
};
use re_viewport_blueprint::ViewProperty;

use crate::graph::Graph;
use crate::layout::{ForceLayoutParams, LayoutRequest};
use crate::ui::{GraphViewState, LevelOfDetail, draw_graph, view_property_force_ui};
use crate::visualizers::{EdgesVisualizer, NodeVisualizer, merge};

#[derive(Default)]
pub struct GraphView;

impl ViewClass for GraphView {
    // State type as described above.

    fn identifier() -> ViewClassIdentifier {
        "Graph".into()
    }

    fn display_name(&self) -> &'static str {
        "Graph"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_GRAPH
    }

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        let egui::InputOptions { zoom_modifier, .. } = egui::InputOptions::default(); // This is OK, since we don't allow the user to change this modifier.

        Help::new("Graph view")
            .docs_link("https://rerun.io/docs/reference/types/views/graph_view")
            .control("Pan", (MouseButtonText(DRAG_PAN2D_BUTTON), "+", "drag"))
            .control(
                "Zoom",
                IconText::from_modifiers_and(os, zoom_modifier, icons::SCROLL),
            )
            .control("Reset view", ("double", icons::LEFT_MOUSE_CLICK))
    }

    /// Register all systems (contexts & parts) that the view needs.
    fn on_register(
        &self,
        system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        fn valid_bound(rect: &egui::Rect) -> bool {
            rect.is_finite() && rect.is_positive()
        }

        system_registry.register_fallback_provider(
            VisualBounds2D::descriptor_range().component,
            |ctx| {
                let Ok(state) = ctx.view_state().downcast_ref::<GraphViewState>() else {
                    return re_sdk_types::blueprint::components::VisualBounds2D::default();
                };

                match state.layout_state.bounding_rect() {
                    Some(rect) if valid_bound(&rect) => rect.into(),
                    _ => re_sdk_types::blueprint::components::VisualBounds2D::default(),
                }
            },
        );

        // ForceManyBody
        system_registry.register_fallback_provider(
            blueprint::archetypes::ForceManyBody::descriptor_strength().component,
            |_| blueprint::components::ForceStrength::from(-60.),
        );
        system_registry.register_fallback_provider(
            blueprint::archetypes::ForceManyBody::descriptor_enabled().component,
            |_| blueprint::components::Enabled::from(true),
        );

        // ForcePosition
        system_registry.register_fallback_provider(
            blueprint::archetypes::ForcePosition::descriptor_strength().component,
            |_| blueprint::components::ForceStrength::from(0.01),
        );
        system_registry.register_fallback_provider(
            blueprint::archetypes::ForcePosition::descriptor_enabled().component,
            |_| blueprint::components::Enabled::from(true),
        );

        // ForceLink
        system_registry.register_fallback_provider(
            blueprint::archetypes::ForceLink::descriptor_enabled().component,
            |_| blueprint::components::Enabled::from(true),
        );
        system_registry.register_fallback_provider(
            blueprint::archetypes::ForceLink::descriptor_iterations().component,
            |_| blueprint::components::ForceIterations::from(3),
        );

        // ForceCollisionRadius
        system_registry.register_fallback_provider(
            blueprint::archetypes::ForceCollisionRadius::descriptor_iterations().component,
            |_| blueprint::components::ForceIterations::from(1),
        );

        system_registry.register_visualizer::<NodeVisualizer>()?;
        system_registry.register_visualizer::<EdgesVisualizer>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<GraphViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, state: &dyn ViewState) -> Option<f32> {
        let state = state.downcast_ref::<GraphViewState>().ok()?;

        if let Some(bounds) = state.visual_bounds {
            let width = bounds.x_range.abs_len() as f32;
            let height = bounds.y_range.abs_len() as f32;
            return Some(width / height);
        }

        if let Some(rect) = state.layout_state.bounding_rect() {
            let width = rect.width().abs();
            let height = rect.height().abs();
            return Some(width / height);
        }
        None
    }

    fn layout_priority(&self) -> ViewClassLayoutPriority {
        Default::default()
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        suggest_view_for_each_entity::<NodeVisualizer>(ctx, include_entity)
    }

    /// Additional UI displayed when the view is selected.
    ///
    /// In this sample we show a combo box to select the color coordinates mode.
    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<GraphViewState>()?;

        ui.selection_grid("graph_view_settings_ui").show(ui, |ui| {
            state.layout_ui(ui);
            state.simulation_ui(ui);
        });

        re_ui::list_item::list_item_scope(ui, "graph_selection_ui", |ui| {
            let ctx = self.view_context(ctx, view_id, state, space_origin);
            view_property_ui::<GraphBackground>(&ctx, ui);
            view_property_ui::<VisualBounds2D>(&ctx, ui);
            view_property_force_ui::<ForceLink>(&ctx, ui);
            view_property_force_ui::<ForceManyBody>(&ctx, ui);
            view_property_force_ui::<ForcePosition>(&ctx, ui);
            view_property_force_ui::<ForceCenter>(&ctx, ui);
            view_property_force_ui::<ForceCollisionRadius>(&ctx, ui);
        });

        Ok(())
    }

    /// The contents of the View window and all interaction within it.
    ///
    /// This is called with freshly created & executed context & part systems.
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let node_data = &system_output.view_systems.get::<NodeVisualizer>()?.data;
        let edge_data = &system_output.view_systems.get::<EdgesVisualizer>()?.data;

        let graphs = merge(node_data, edge_data)
            .map(|(ent, nodes, edges)| Graph::new(ui, ent.clone(), nodes, edges))
            .collect::<Vec<_>>();

        let state = state.downcast_mut::<GraphViewState>()?;

        let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);
        let params = ForceLayoutParams::get(&view_ctx)?;

        let background = ViewProperty::from_archetype::<GraphBackground>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let background_color = background.component_or_fallback::<Color>(
            &view_ctx,
            GraphBackground::descriptor_color().component,
        )?;

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let rect_in_scene: blueprint::components::VisualBounds2D = bounds_property
            .component_or_fallback(&view_ctx, VisualBounds2D::descriptor_range().component)?;

        // Perform all layout-related tasks.
        let request = LayoutRequest::from_graphs(graphs.iter());
        let layout = state.layout_state.get(request, params);

        let mut scene_rect = egui::Rect::from(rect_in_scene);
        let scene_rect_ref = scene_rect;

        // To determine the overall scale factor needed for the level-of-details
        // computation, we need to look at the two dimensions separately due to letter-boxing.
        let rect_in_ui = ui.max_rect();
        let scale = rect_in_ui.size() / scene_rect.size();

        let level_of_detail = LevelOfDetail::from_scaling(scale.min_elem());

        let mut hover_click_item: Option<(Item, egui::Response)> = None;

        ui.painter().rect_filled(rect_in_ui, 0.0, background_color);

        let resp = egui::Scene::new()
            .show(ui, &mut scene_rect, |ui| {
                for graph in &graphs {
                    draw_graph(
                        ui,
                        ctx,
                        graph,
                        layout,
                        query,
                        level_of_detail,
                        &mut hover_click_item,
                    );
                }
            })
            .response;

        if let Some((item, response)) = hover_click_item {
            ctx.handle_select_hover_drag_interactions(&response, item, false);
        } else if resp.hovered() {
            ctx.selection_state().set_hovered(Item::View(query.view_id));
        }

        if resp.clicked() {
            // clicked elsewhere, select the view
            ctx.command_sender()
                .send_system(SystemCommand::set_selection(Item::View(query.view_id)));
        }

        // Update blueprint if changed
        let updated_bounds = blueprint::components::VisualBounds2D::from(scene_rect);
        if resp.double_clicked() {
            bounds_property.reset_blueprint_component(
                ctx,
                blueprint::archetypes::VisualBounds2D::descriptor_range(),
            );
        } else if scene_rect != scene_rect_ref {
            bounds_property.save_blueprint_component(
                ctx,
                &VisualBounds2D::descriptor_range(),
                &updated_bounds,
            );
        }
        // Update stored bounds on the state, so visualizers see an up-to-date value.
        state.visual_bounds = Some(updated_bounds);

        if state.layout_state.is_in_progress() {
            ui.request_repaint();
        }

        Ok(())
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| GraphView.help(ctx));
}
