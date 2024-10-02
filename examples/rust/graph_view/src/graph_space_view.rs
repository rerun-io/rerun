use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};
use std::collections::HashMap;

use re_viewer::external::{
    arrow2::compute,
    egui::{self, emath::TSTransform, Rect, TextWrapMode},
    re_log::external::log,
    re_log_types::EntityPath,
    re_types::SpaceViewClassIdentifier,
    re_ui,
    re_viewer_context::{
        HoverHighlight, IdentifiedViewSystem as _, InteractionHighlight, SelectionHighlight,
        SpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
        SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
        SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput,
        ViewQuery, ViewerContext,
    },
};

use crate::{
    common::QualifiedEdge,
    node_visualizer_system::{GraphNodeVisualizer, NodeInstance},
};
use crate::{common::QualifiedNode, edge_visualizer_system::GraphEdgeVisualizer};

fn measure_node_sizes<'a>(
    ui: &mut egui::Ui,
    nodes: impl Iterator<Item = NodeInstance<'a>>,
) -> Vec<(QualifiedNode, egui::Vec2)> {
    let mut sizes = Vec::new();
    let ctx = ui.ctx();
    ctx.request_discard("measuring node sizes");
    ui.horizontal(|ui| {
        for node in nodes {
            let response = node.draw(ui, InteractionHighlight::default());
            sizes.push((node.node_id.clone(), response.rect.size()));
        }
    });
    sizes
}

fn compute_layout(
    nodes: impl Iterator<Item = (QualifiedNode, egui::Vec2)>,
    edges: impl Iterator<Item = QualifiedEdge>,
) -> HashMap<QualifiedNode, egui::Rect> {
    let mut node_to_index = HashMap::new();
    let mut graph: ForceGraph<NodeKind, ()> = ForceGraph::default();

    // TODO(grtlr): `fdg` does not account for node sizes out of the box.
    for (node_id, size) in nodes {
        let ix = graph.add_force_node(
            node_id.to_string(),
            NodeKind::Regular(node_id.clone(), size),
        );
        node_to_index.insert(node_id, ix);
    }

    for QualifiedEdge { source, target } in edges {
        let source_ix = *node_to_index
            .entry(source.clone())
            .or_insert(graph.add_force_node(source.to_string(), NodeKind::Dummy(source)));
        let target_ix = *node_to_index
            .entry(target.clone())
            .or_insert(graph.add_force_node(target.to_string(), NodeKind::Dummy(target)));
        graph.add_edge(source_ix, target_ix, ());
    }

    // create a simulation from the graph
    let mut simulation = Simulation::from_graph(graph, SimulationParameters::default());

    for frame in 0..1000 {
        simulation.update(0.035);
    }

    simulation
        .get_graph()
        .node_weights()
        .filter_map(|node| match &node.data {
            NodeKind::Regular(node_id, size) => {
                let center = egui::Pos2::new(node.location.x, node.location.y);
                let rect = egui::Rect::from_center_size(center, *size);
                Some((node_id.clone(), rect))
            }
            NodeKind::Dummy(_) => None,
        })
        .collect()
}

fn bounding_rect_from_iter<'a>(rects: impl Iterator<Item = &'a egui::Rect>) -> Option<egui::Rect> {
    // Start with `None` and gradually expand the bounding box.
    let mut bounding_rect: Option<egui::Rect> = None;

    for rect in rects {
        bounding_rect = match bounding_rect {
            Some(bounding) => Some(bounding.union(*rect)),
            None => Some(*rect),
        };
    }

    bounding_rect
}

fn fit_bounding_rect_to_screen(
    bounding_rect: egui::Rect,
    available_size: egui::Vec2,
) -> TSTransform {
    // Compute the scale factor to fit the bounding rectangle into the available screen size.
    let scale_x = available_size.x / bounding_rect.width();
    let scale_y = available_size.y / bounding_rect.height();

    // Use the smaller of the two scales to ensure the whole rectangle fits on the screen.
    let scale = scale_x.min(scale_y);

    // Compute the translation to center the bounding rect in the screen.
    let center_screen = egui::Pos2::new(available_size.x / 2.0, available_size.y / 2.0);
    let center_world = bounding_rect.center().to_vec2();

    // Set the transformation to scale and then translate to center.
    TSTransform::from_translation(center_screen.to_vec2() - center_world * scale)
        * TSTransform::from_scaling(scale)
}

// We need to differentiate between regular nodes and nodes that belong to a different entity hierarchy.
enum NodeKind {
    Regular(QualifiedNode, egui::Vec2),
    Dummy(QualifiedNode),
}

impl<'a> NodeInstance<'a> {
    fn text(&self) -> egui::RichText {
        self.label.map_or(
            egui::RichText::new(format!("{}@{}", self.node_id.node_id, self.entity_path)),
            |label| egui::RichText::new(label.to_string()),
        )
    }

    fn draw(&self, ui: &mut egui::Ui, highlight: InteractionHighlight) -> egui::Response {
        let hcolor = match (
            highlight.hover,
            highlight.selection != SelectionHighlight::None,
        ) {
            (HoverHighlight::None, false) => ui.style().visuals.text_color(),
            (HoverHighlight::None, true) => ui.style().visuals.selection.bg_fill,
            (HoverHighlight::Hovered, ..) => ui.style().visuals.widgets.hovered.bg_fill,
        };

        egui::Frame::default()
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(
                1.0,
                if highlight.selection == SelectionHighlight::Selection {
                    ui.style().visuals.selection.bg_fill
                } else {
                    ui.ctx().style().visuals.text_color()
                },
            ))
            .fill(ui.style().visuals.faint_bg_color)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                if let Some(color) = self.color {
                    ui.button(self.text().color(color))
                } else {
                    ui.button(self.text())
                }
            })
            .response
    }
}

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct GraphSpaceViewState {
    screen_to_world: TSTransform,

    /// Positions of the nodes in world space.
    layout: Option<HashMap<QualifiedNode, egui::Rect>>,
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
        let (id, rect) = ui.allocate_space(ui.available_size());

        let Some(layout) = &mut state.layout else {
            let node_sizes =
                measure_node_sizes(ui, node_system.data.iter().flat_map(|d| d.nodes()));

            let layout = compute_layout(
                node_sizes.into_iter(),
                edge_system
                    .data
                    .iter()
                    .flat_map(|d| d.edges().map(|(edge, _, _)| edge)),
            );

            if let Some(bounding_box) = bounding_rect_from_iter(layout.values()) {
                state.screen_to_world =
                    fit_bounding_rect_to_screen(bounding_box.scale_from_center(1.05), rect.size());
            }

            state.layout = Some(layout);

            return Ok(());
        };

        let response = ui.interact(rect, id, egui::Sense::click_and_drag());

        // Allow dragging the background as well.
        if response.dragged() {
            state.screen_to_world.translation += response.drag_delta();
        }

        let transform = TSTransform::from_translation(ui.min_rect().left_top().to_vec2())
            * state.screen_to_world;

        #[cfg(debug_assertions)]
        if response.double_clicked() {
            if let Some(screen) = response.interact_pointer_pos() {
                log::debug!(
                    "Clicked! Screen: {:?}, World: {:?}",
                    screen,
                    transform.inverse() * screen
                );
            }
        }

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

        let window_layer = ui.layer_id();

        #[cfg(debug_assertions)]
        {
            log::debug!("Displaying coordinate system");
            // paint coordinate system at the world origin
            let origin = transform * egui::Pos2::new(0.0, 0.0);
            let x_axis = transform * egui::Pos2::new(100.0, 0.0);
            let y_axis = transform * egui::Pos2::new(0.0, 100.0);

            // Paint the coordinate system.
            let painter = egui::Painter::new(
                ui.ctx().clone(),
                window_layer,
                /* transform.inverse() * */ rect,
            );
            painter.line_segment([origin, x_axis], egui::Stroke::new(1.0, egui::Color32::RED));
            painter.line_segment(
                [origin, y_axis],
                egui::Stroke::new(2.0, egui::Color32::GREEN),
            );

            if let Some(bounding_box) = bounding_rect_from_iter(layout.values()) {
                log::debug!("Node bounding box: {:?}", bounding_box);

                painter.rect(
                    transform * bounding_box,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(255, 0, 255, 32),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 0, 255)),
                );
            }
        }

        for data in node_system.data.iter() {
            let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());
            let mut entity_rect: Option<Rect> = None;

            for node in data.nodes() {
                let current_extent = layout
                    .get(&node.node_id)
                    .expect("missing layout information for node");
                let response = egui::Area::new(id.with((node.node_id.clone(), node.instance)))
                    .current_pos(current_extent.min)
                    .order(egui::Order::Middle)
                    .constrain(false)
                    .show(ui.ctx(), |ui| {
                        let highlight = ent_highlight.index_highlight(node.instance);
                        ui.set_clip_rect(transform.inverse() * rect);
                        node.draw(ui, highlight)
                    })
                    .response;

                entity_rect =
                    entity_rect.map_or(Some(response.rect), |r| Some(r.union(response.rect)));
                layout.insert(node.node_id.clone(), response.rect);

                let id = response.layer_id;

                ui.ctx().set_transform_layer(id, transform);
                ui.ctx().set_sublayer(window_layer, id);

                // ui.interact(response.rect, area_id, egui::Sense::click());
            }

            let entity_path = data.entity_path.clone();
            if let Some(entity_rect) = entity_rect {
                let response = egui::Area::new(id.with(entity_path.clone()))
                    .fixed_pos(entity_rect.min)
                    .order(egui::Order::Background)
                    .show(ui.ctx(), |ui| {
                        ui.set_clip_rect(transform.inverse() * rect);
                        egui::Frame::default()
                            .rounding(egui::Rounding::same(4.0))
                            .stroke(egui::Stroke::new(
                                1.0,
                                ui.ctx().style().visuals.text_color(),
                            ))
                            .fill(ui.style().visuals.faint_bg_color)
                            .show(ui, |ui| {
                                ui.allocate_exact_size(entity_rect.size(), egui::Sense::hover());
                                ui.label(format!("{}", entity_path))
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
                if let (Some(source_pos), Some(target_pos)) =
                    (layout.get(&edge.source), layout.get(&edge.target))
                {
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
                            ui.set_clip_rect(transform.inverse() * rect);
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
