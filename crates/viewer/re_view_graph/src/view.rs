use re_log_types::EntityPath;
use re_types::{
    ViewClassIdentifier,
    blueprint::{
        self,
        archetypes::{
            ForceCenter, ForceCollisionRadius, ForceLink, ForceManyBody, ForcePosition,
            VisualBounds2D,
        },
    },
};
use re_ui::{self, Help, MouseButtonText, UiExt as _, icon_text, icons, shortcut_with_icon};
use re_view::{
    controls::{DRAG_PAN2D_BUTTON, ZOOM_SCROLL_MODIFIER},
    view_property_ui,
};
use re_viewer_context::{
    IdentifiedViewSystem as _, Item, RecommendedView, SystemExecutionOutput, ViewClass,
    ViewClassLayoutPriority, ViewClassRegistryError, ViewId, ViewQuery, ViewSpawnHeuristics,
    ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewSystemRegistrator, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use crate::{
    graph::Graph,
    layout::{ForceLayoutParams, LayoutRequest},
    ui::{GraphViewState, LevelOfDetail, draw_graph, view_property_force_ui},
    visualizers::{EdgesVisualizer, NodeVisualizer, merge},
};

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

    fn help(&self, egui_ctx: &egui::Context) -> Help {
        Help::new("Graph view")
            .docs_link("https://rerun.io/docs/reference/types/views/graph_view")
            .control(
                "Pan",
                icon_text!(MouseButtonText(DRAG_PAN2D_BUTTON), "+", "drag"),
            )
            .control(
                "Zoom",
                shortcut_with_icon(egui_ctx, ZOOM_SCROLL_MODIFIER, icons::SCROLL),
            )
            .control("Reset view", icon_text!("double", icons::LEFT_MOUSE_CLICK))
    }

    /// Register all systems (contexts & parts) that the view needs.
    fn on_register(
        &self,
        system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
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
        // TODO(grtlr): Consider using `suggest_view_for_each_entity` here too.
        if let Some(maybe_visualizable) = ctx
            .maybe_visualizable_entities_per_visualizer
            .get(&NodeVisualizer::identifier())
        {
            ViewSpawnHeuristics::new(maybe_visualizable.iter().cloned().filter_map(|entity| {
                if include_entity(&entity) {
                    Some(RecommendedView::new_single_entity(entity))
                } else {
                    None
                }
            }))
        } else {
            ViewSpawnHeuristics::empty()
        }
    }

    /// Additional UI displayed when the view is selected.
    ///
    /// In this sample we show a combo box to select the color coordinates mode.
    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<GraphViewState>()?;

        ui.selection_grid("graph_view_settings_ui").show(ui, |ui| {
            state.layout_ui(ui);
            state.simulation_ui(ui);
        });

        re_ui::list_item::list_item_scope(ui, "graph_selection_ui", |ui| {
            view_property_ui::<VisualBounds2D>(ctx, ui, view_id, self, state);
            view_property_force_ui::<ForceLink>(ctx, ui, view_id, self, state);
            view_property_force_ui::<ForceManyBody>(ctx, ui, view_id, self, state);
            view_property_force_ui::<ForcePosition>(ctx, ui, view_id, self, state);
            view_property_force_ui::<ForceCenter>(ctx, ui, view_id, self, state);
            view_property_force_ui::<ForceCollisionRadius>(ctx, ui, view_id, self, state);
        });

        Ok(())
    }

    /// The contents of the View window and all interaction within it.
    ///
    /// This is called with freshly created & executed context & part systems.
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
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

        let params = ForceLayoutParams::get(ctx, query, self, state)?;

        let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let rect_in_scene: blueprint::components::VisualBounds2D = bounds_property
            .component_or_fallback(ctx, self, state, &VisualBounds2D::descriptor_range())?;

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
            ctx.selection_state()
                .set_selection(Item::View(query.view_id));
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
            ui.ctx().request_repaint();
        }

        Ok(())
    }
}

#[test]
fn test_help_view() {
    re_viewer_context::test_context::TestContext::test_help_view(|ctx| GraphView.help(ctx));
}
