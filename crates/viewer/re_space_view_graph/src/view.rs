use re_log_types::EntityPath;
use re_space_view::{
    controls::{DRAG_PAN2D_BUTTON, ZOOM_SCROLL_MODIFIER},
    view_property_ui,
};
use re_types::{
    blueprint::{self, archetypes::VisualBounds2D},
    SpaceViewClassIdentifier,
};
use re_ui::{
    self,
    zoom_pan_area::{fit_to_rect_in_scene, zoom_pan_area},
    ModifiersMarkdown, MouseButtonMarkdown, UiExt as _,
};
use re_viewer_context::{
    IdentifiedViewSystem as _, RecommendedSpaceView, SpaceViewClass, SpaceViewClassLayoutPriority,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewState,
    SpaceViewStateExt as _, SpaceViewSystemExecutionError, SpaceViewSystemRegistrator,
    SystemExecutionOutput, ViewQuery, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::{
    graph::Graph,
    layout::LayoutRequest,
    ui::{draw_debug, draw_graph, GraphSpaceViewState},
    visualizers::{merge, EdgesVisualizer, NodeVisualizer},
};

#[derive(Default)]
pub struct GraphSpaceView;

impl SpaceViewClass for GraphSpaceView {
    // State type as described above.

    fn identifier() -> SpaceViewClassIdentifier {
        "Graph".into()
    }

    fn display_name(&self) -> &'static str {
        "Graph"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_GRAPH
    }

    fn help_markdown(&self, egui_ctx: &egui::Context) -> String {
        format!(
            r"# Graph View

Display a graph of nodes and edges.

## Navigation controls
- Pinch gesture or {zoom_scroll_modifier} + scroll to zoom.
- Click and drag with the {drag_pan2d_button} to pan.
- Double-click to reset the view.",
            zoom_scroll_modifier = ModifiersMarkdown(ZOOM_SCROLL_MODIFIER, egui_ctx),
            drag_pan2d_button = MouseButtonMarkdown(DRAG_PAN2D_BUTTON),
        )
    }

    /// Register all systems (contexts & parts) that the space view needs.
    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<NodeVisualizer>()?;
        system_registry.register_visualizer::<EdgesVisualizer>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<GraphSpaceViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, state: &dyn SpaceViewState) -> Option<f32> {
        let state = state.downcast_ref::<GraphSpaceViewState>().ok()?;

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

    fn layout_priority(&self) -> SpaceViewClassLayoutPriority {
        Default::default()
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        if let Some(applicable) = ctx
            .applicable_entities_per_visualizer
            .get(&NodeVisualizer::identifier())
        {
            SpaceViewSpawnHeuristics::new(
                applicable
                    .iter()
                    .cloned()
                    .map(RecommendedSpaceView::new_single_entity),
            )
        } else {
            SpaceViewSpawnHeuristics::empty()
        }
    }

    /// Additional UI displayed when the space view is selected.
    ///
    /// In this sample we show a combo box to select the color coordinates mode.
    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        ui.selection_grid("graph_view_settings_ui").show(ui, |ui| {
            state.layout_ui(ui);
            state.simulation_ui(ui);
            state.debug_ui(ui);
        });

        view_property_ui::<VisualBounds2D>(ctx, ui, space_view_id, self, state);

        Ok(())
    }

    /// The contents of the Space View window and all interaction within it.
    ///
    /// This is called with freshly created & executed context & part systems.
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let node_data = &system_output.view_systems.get::<NodeVisualizer>()?.data;
        let edge_data = &system_output.view_systems.get::<EdgesVisualizer>()?.data;

        let graphs = merge(node_data, edge_data)
            .map(|(ent, nodes, edges)| Graph::new(ui, ent.clone(), nodes, edges))
            .collect::<Vec<_>>();

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );
        let rect_in_scene: blueprint::components::VisualBounds2D =
            bounds_property.component_or_fallback(ctx, self, state)?;

        let rect_in_ui = *state.rect_in_ui.insert(ui.max_rect());

        let request = LayoutRequest::from_graphs(graphs.iter());
        let layout_was_empty = state.layout_state.is_none();
        let layout = state.layout_state.get(request);

        let ui_from_world = state
            .ui_from_world
            .get_or_insert_with(|| fit_to_rect_in_scene(rect_in_ui, rect_in_scene.into()));

        let resp = zoom_pan_area(ui, rect_in_ui, ui_from_world, |ui| {
            let mut world_bounding_rect = egui::Rect::NOTHING;

            for graph in &graphs {
                let graph_rect = draw_graph(ui, ctx, graph, layout, query);
                world_bounding_rect = world_bounding_rect.union(graph_rect);
            }

            // We need to draw the debug information after the rest to ensure that we have the correct bounding box.
            if state.show_debug {
                draw_debug(ui, world_bounding_rect);
            }
        });

        // Update blueprint if changed
        let updated_rect_in_scene =
            blueprint::components::VisualBounds2D::from(ui_from_world.inverse() * rect_in_ui);
        if resp.double_clicked() || layout_was_empty {
            bounds_property.reset_blueprint_component::<blueprint::components::VisualBounds2D>(ctx);
            state.ui_from_world = None;
        } else if rect_in_scene != updated_rect_in_scene {
            bounds_property.save_blueprint_component(ctx, &updated_rect_in_scene);
        }
        // Update stored bounds on the state, so visualizers see an up-to-date value.
        state.visual_bounds = Some(updated_rect_in_scene);

        if state.layout_state.is_in_progress() {
            ui.ctx().request_repaint();
        }

        Ok(())
    }
}
