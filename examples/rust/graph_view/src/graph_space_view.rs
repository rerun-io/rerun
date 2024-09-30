use std::{collections::HashMap, hash::Hash};

use re_log_types::Instance;
use re_viewer::external::{
    egui::{
        self,
        emath::{TSTransform, Vec2},
        Color32, Label, Rect, RichText, TextWrapMode,
    },
    re_entity_db::InstancePath,
    re_log::external::log,
    re_log_types::EntityPath,
    re_types::{
        components::{self, PoseRotationAxisAngle},
        ArrowString, SpaceViewClassIdentifier,
    },
    re_ui,
    re_viewer_context::{
        HoverHighlight, IdentifiedViewSystem as _, OptionalSpaceViewEntityHighlight,
        SelectionHighlight, SpaceViewClass, SpaceViewClassLayoutPriority,
        SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewState,
        SpaceViewStateExt as _, SpaceViewSystemExecutionError, SpaceViewSystemRegistrator,
        SystemExecutionOutput, ViewQuery, ViewerContext,
    },
};

use crate::node_visualizer_system::GraphNodeVisualizer;
use crate::{common::QualifiedNode, edge_visualizer_system::GraphEdgeVisualizer};

// We need to differentiate between regular nodes and nodes that belong to a different entity hierarchy.
enum NodeKind {
    Regular(QualifiedNode),
    Dummy(QualifiedNode),
}

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct GraphSpaceViewState {
    graph: petgraph::stable_graph::StableGraph<NodeKind, ()>,
    node_to_index: HashMap<QualifiedNode, petgraph::stable_graph::NodeIndex>,
    // graph viewer
    screen_to_world: TSTransform,
    dragging: Option<QualifiedNode>,
    /// Positions of the nodes in world space.
    // We currently store position and size, but should maybe store the actual rectangle in the future.
    node_positions: HashMap<QualifiedNode, egui::Rect>,
}

impl SpaceViewState for GraphSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

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
        &re_ui::icons::SPACE_VIEW_GENERIC
    }

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "A space view that shows a graph as a node link diagram.".to_owned()
    }

    /// Register all systems (contexts & parts) that the space view needs.
    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<GraphNodeVisualizer>()?;
        system_registry.register_visualizer::<GraphEdgeVisualizer>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<GraphSpaceViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    fn layout_priority(&self) -> SpaceViewClassLayoutPriority {
        Default::default()
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        // By default spawn a single view at the root if there's anything the visualizer is applicable to.
        if ctx
            .applicable_entities_per_visualizer
            .get(&GraphNodeVisualizer::identifier())
            .map_or(true, |entities| entities.is_empty())
        {
            SpaceViewSpawnHeuristics::default()
        } else {
            SpaceViewSpawnHeuristics::root()
        }
    }

    /// Additional UI displayed when the space view is selected.
    ///
    /// In this sample we show a combo box to select the color coordinates mode.
    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let _state = state.downcast_mut::<GraphSpaceViewState>()?;

        ui.horizontal(|ui| {
            ui.label("HEEEEELLLLLLOOOOO");
        });

        Ok(())
    }

    /// The contents of the Space View window and all interaction within it.
    ///
    /// This is called with freshly created & executed context & part systems.
    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let node_system = system_output.view_systems.get::<GraphNodeVisualizer>()?;
        let edge_system = system_output.view_systems.get::<GraphEdgeVisualizer>()?;

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        // TODO(grtlr): Once we settle on a design, we should update the graph instead of constructing it from scratch.
        state.graph.clear();
        state.node_to_index.clear();

        for data in node_system.data.iter() {
            for (node_id, _, _, _) in data.nodes() {
                let node_index = state.graph.add_node(NodeKind::Regular(node_id.clone()));
                state.node_to_index.insert(node_id, node_index);
            }
        }

        for data in edge_system.data.iter() {
            for (edge, _, _) in data.edges() {
                let source_index = *state
                    .node_to_index
                    .entry(edge.source.clone())
                    .or_insert(state.graph.add_node(NodeKind::Dummy(edge.source)));
                let target_index = *state
                    .node_to_index
                    .entry(edge.target.clone())
                    .or_insert(state.graph.add_node(NodeKind::Dummy(edge.target)));
                state.graph.add_edge(source_index, target_index, ());
            }
        }

        // Graph viewer
        let (id, rect) = ui.allocate_space(ui.available_size());
        let response = ui.interact(rect, id, egui::Sense::click_and_drag());

        // Allow dragging the background as well.
        if response.dragged() {
            state.screen_to_world.translation += response.drag_delta();
        }

        // Plot-like reset
        if response.double_clicked() {
            state.screen_to_world = TSTransform::default();
        }

        let transform = TSTransform::from_translation(ui.min_rect().left_top().to_vec2())
            * state.screen_to_world;

        if let Some(pointer) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            // Note: doesn't catch zooming / panning if a button in this PanZoom container is hovered.
            if response.hovered() {
                let pointer_in_world = transform.inverse() * pointer;
                let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
                let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

                // Zoom in on pointer:
                state.screen_to_world = state.screen_to_world
                    * TSTransform::from_translation(pointer_in_world.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_world.to_vec2());

                // Pan:
                state.screen_to_world =
                    TSTransform::from_translation(pan_delta) * state.screen_to_world;
            }
        }

        // initial layout
        let mut positions = (0..).map(|i| egui::Pos2::new(0.0, 0.0 + i as f32 * 30.0));
        let window_layer = ui.layer_id();

        for data in node_system.data.iter() {
            let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());
            let mut entity_rect: Option<Rect> = None;

            for (i, (node, instance, maybe_color, maybe_label)) in data.nodes().enumerate() {
                let area_id = id.with((node.clone(), i));
                let response = egui::Area::new(area_id)
                    .current_pos(
                        state
                            .node_positions
                            .get(&node)
                            .map_or(positions.next().unwrap(), |r| r.min),
                    )
                    .order(egui::Order::Middle)
                    .constrain(false)
                    .show(ui.ctx(), |ui| {
                        ui.set_clip_rect(transform.inverse() * rect);
                        egui::Frame::default()
                            .rounding(egui::Rounding::same(4.0))
                            .inner_margin(egui::Margin::same(8.0))
                            .stroke(egui::Stroke::new(
                                1.0,
                                ui.ctx().style().visuals.text_color(),
                            ))
                            .fill(ui.style().visuals.faint_bg_color)
                            .show(ui, |ui| {
                                ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);

                                let highlight = ent_highlight.index_highlight(instance);

                                let hcolor = match (
                                    highlight.hover,
                                    highlight.selection != SelectionHighlight::None,
                                ) {
                                    (HoverHighlight::None, false) => egui::Color32::BLACK,
                                    (HoverHighlight::None, true) => {
                                        ui.style().visuals.selection.bg_fill
                                    }
                                    (HoverHighlight::Hovered, ..) => {
                                        ui.style().visuals.widgets.hovered.bg_fill
                                    }
                                };

                                let text = if let Some(label) = maybe_label {
                                    egui::RichText::new(format!("{}", label))
                                } else {
                                    egui::RichText::new(format!(
                                        "{}:{}",
                                        node.entity_path, node.node_id,
                                    ))
                                };

                                if let Some(color) = maybe_color {
                                    let c = Color32::from(color.0);
                                    ui.button(text.color(c).background_color(hcolor))
                                } else {
                                    ui.button(text.background_color(hcolor))
                                }
                            });
                    })
                    .response;

                entity_rect =
                    entity_rect.map_or(Some(response.rect), |r| Some(r.union(response.rect)));
                state.node_positions.insert(node.clone(), response.rect);

                let id = response.layer_id;

                ui.ctx().set_transform_layer(id, transform);
                ui.ctx().set_sublayer(window_layer, id);

                // ui.interact(response.rect, area_id, egui::Sense::click());
            }

            let entity_path = data.entity_path.clone();
            if let Some(entity_rect) = entity_rect {
                let response = egui::Area::new(id.with(entity_path.clone()))
                    .current_pos(entity_rect.min)
                    .order(egui::Order::Background)
                    .show(ui.ctx(), |ui| {
                        ui.set_clip_rect(transform.inverse() * rect);
                        egui::Frame::default()
                            .rounding(egui::Rounding::same(4.0))
                            .inner_margin(egui::Margin::same(8.0))
                            .stroke(egui::Stroke::new(
                                1.0,
                                ui.ctx().style().visuals.text_color(),
                            ))
                            .fill(ui.style().visuals.faint_bg_color)
                            .show(ui, |ui| {
                                ui.label(format!("{}", entity_path));
                                ui.allocate_exact_size(entity_rect.size(), egui::Sense::hover())
                            });
                    })
                    .response;

                let layer_id = response.layer_id;
                ui.ctx().set_transform_layer(layer_id, transform);
                ui.ctx().set_sublayer(window_layer, layer_id);
            }
        }

        for data in edge_system.data.iter() {
            let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

            for (i, (edge, instance, color)) in data.edges().enumerate() {
                // TODO(grtlr): This does not handle dummy nodes correctly.
                if let (Some(source_pos), Some(target_pos)) = (
                    state.node_positions.get(&edge.source),
                    state.node_positions.get(&edge.target),
                ) {
                    let highlight = ent_highlight.index_highlight(instance);

                    let hcolor = match (
                        highlight.hover,
                        highlight.selection != SelectionHighlight::None,
                    ) {
                        (HoverHighlight::None, false) => ui.style().visuals.text_color(),
                        (HoverHighlight::None, true) => ui.style().visuals.selection.bg_fill,
                        (HoverHighlight::Hovered, ..) => ui.style().visuals.widgets.hovered.bg_fill,
                    };

                    let response = egui::Area::new(id.with((edge, i)))
                        .current_pos(source_pos.center())
                        .order(egui::Order::Middle)
                        .constrain(false)
                        .show(ui.ctx(), |ui| {
                            // TODO(grtlr): reintroduce clipping: `ui.set_clip_rect(transform.inverse() * rect);`
                            egui::Frame::default().show(ui, |ui| {
                                let painter = ui.painter();
                                painter.line_segment(
                                    [source_pos.center(), target_pos.center()],
                                    egui::Stroke::new(2.0, hcolor),
                                );
                            });

                            // log::debug!("Line: {} {}", source_pos, target_pos);
                        })
                        .response;

                    let id = response.layer_id;

                    ui.ctx().set_transform_layer(id, transform);
                    ui.ctx().set_sublayer(window_layer, id);
                }
            }
        }

        Ok(())
    }
}
