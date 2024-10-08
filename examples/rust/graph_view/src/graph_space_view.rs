use fdg_sim::{ForceGraph, ForceGraphHelper, Simulation, SimulationParameters};
use std::collections::{HashMap, HashSet};

use re_viewer::external::{
    egui::{self, emath::TSTransform, TextWrapMode},
    re_entity_db::InstancePath,
    re_log::external::log,
    re_log_types::EntityPath,
    re_types::{datatypes, SpaceViewClassIdentifier},
    re_ui,
    re_viewer_context::{
        HoverHighlight, IdentifiedViewSystem as _, InteractionHighlight, Item, SelectionHighlight,
        SpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
        SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
        SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput,
        ViewQuery, ViewerContext,
    },
};

use crate::{
    common::NodeLocation,
    edge_undirected_visualizer_system::EdgeUndirectedVisualizer,
    graph::{Graph, Node},
};
use crate::{
    edge_undirected_visualizer_system::{self, EdgeInstance},
    node_visualizer_system::{GraphNodeVisualizer, NodeInstance},
};

impl<'a> Node<'a> {
    fn draw(&self, ui: &mut egui::Ui, highlight: InteractionHighlight) -> egui::Response {
        match self {
        Node::Regular(node) => node.draw(ui, highlight),
        Node::Dummy(location, entity_path) => {
                draw_dummy(ui, (*entity_path).clone(), location.node_id.clone())
            }
        }
    }

    fn location(&self) -> NodeLocation {
        match self {
            Node::Regular(node) => node.location.clone(),
            Node::Dummy(location, _) => location.clone(),
        }
    }
}

fn measure_node_sizes<'a>(
    ui: &mut egui::Ui,
    nodes: impl Iterator<Item = Node<'a>>,
) -> HashMap<NodeLocation, egui::Vec2> {
    let mut sizes = HashMap::new();
    let ctx = ui.ctx();
    ctx.request_discard("measuring node sizes");
    ui.horizontal(|ui| {
        for node in nodes {
            let response = node.draw(ui, InteractionHighlight::default());
            sizes.insert(node.location(), response.rect.size());
        }
    });
    sizes
}

fn compute_layout(
    nodes: impl Iterator<Item = (NodeLocation, egui::Vec2)>,
    edges: impl Iterator<Item = (NodeLocation, NodeLocation)>,
) -> HashMap<NodeLocation, egui::Rect> {
    let mut node_to_index = HashMap::new();
    let mut graph: ForceGraph<NodeKind, ()> = ForceGraph::default();

    // TODO(grtlr): `fdg` does not account for node sizes out of the box.
    for (node_id, size) in nodes {
        let ix = graph.add_force_node(
            format!("{:?}", node_id),
            NodeKind::Regular(node_id.clone(), size),
        );
        node_to_index.insert(node_id, ix);
    }

    for (source, target) in edges {
        let source_ix = *node_to_index.entry(source.clone()).or_insert(
            graph.add_force_node(format!("{:?}", source), NodeKind::Dummy(source.clone())),
        );
        let target_ix = *node_to_index.entry(target.clone()).or_insert(
            graph.add_force_node(format!("{:?}", target), NodeKind::Dummy(target.clone())),
        );
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
    let scale = scale_x.min(scale_y).min(1.0);

    // Compute the translation to center the bounding rect in the screen.
    let center_screen = egui::Pos2::new(available_size.x / 2.0, available_size.y / 2.0);
    let center_world = bounding_rect.center().to_vec2();

    // Set the transformation to scale and then translate to center.
    TSTransform::from_translation(center_screen.to_vec2() - center_world * scale)
        * TSTransform::from_scaling(scale)
}

// We need to differentiate between regular nodes and nodes that belong to a different entity hierarchy.
enum NodeKind {
    Regular(NodeLocation, egui::Vec2),
    Dummy(NodeLocation),
}

fn draw_dummy(
    ui: &mut egui::Ui,
    entity_path: datatypes::EntityPath,
    node_id: datatypes::GraphNodeId,
) -> egui::Response {
    let text = egui::RichText::new(format!("{} @ {}", node_id, entity_path.0)).color(
        ui.style().visuals.widgets.noninteractive.text_color(),
    );
    ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
    ui.add(egui::Button::new(text))
    // ui.label(text)
    // egui::Frame::default()
    //     .rounding(egui::Rounding::same(4.0))
    //     .stroke(egui::Stroke::new(1.0, ui.style().visuals.text_color()))
    //     .inner_margin(egui::Vec2::new(6.0, 4.0))
    //     .fill(egui::Color32::RED)
    //     .show(ui, |ui| {
    //         ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
    //         ui.add(egui::Button::new(text))
    //     })
    //     .response
}

impl<'a> NodeInstance<'a> {
    fn text(&self) -> egui::RichText {
        self.label.map_or(
            egui::RichText::new(self.location.node_id.to_string()),
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

        let bg = match highlight.hover {
            HoverHighlight::None => ui.style().visuals.widgets.noninteractive.bg_fill,
            HoverHighlight::Hovered => ui.style().visuals.widgets.hovered.bg_fill,
        };
        // ui.style().visuals.faint_bg_color

        egui::Frame::default()
            .rounding(egui::Rounding::same(4.0))
            .stroke(egui::Stroke::new(1.0, ui.style().visuals.text_color()))
            .inner_margin(egui::Vec2::new(6.0, 4.0))
            .fill(bg)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                if let Some(color) = self.color {
                    ui.add(egui::Button::new(self.text().color(color)));
                } else {
                    ui.add(egui::Button::new(self.text()));
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
    world_to_view: TSTransform,

    /// Positions of the nodes in world space.
    layout: Option<HashMap<NodeLocation, egui::Rect>>,
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
        system_registry.register_visualizer::<EdgeUndirectedVisualizer>()
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
        ctx: &ViewerContext<'_>,
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
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let node_system = system_output.view_systems.get::<GraphNodeVisualizer>()?;
        let edge_system = system_output
            .view_systems
            .get::<EdgeUndirectedVisualizer>()?;

        let state = state.downcast_mut::<GraphSpaceViewState>()?;
        let (id, clip_rect_window) = ui.allocate_space(ui.available_size());

        let Some(layout) = &mut state.layout else {
            let graph = Graph::new(
                node_system.data.iter().flat_map(|d| d.nodes()),
                edge_system.data.iter().flat_map(|d| d.edges()),
            );

            let mut node_sizes = measure_node_sizes(ui, graph.nodes());

            // for data in edge_system.data.iter().flat_map(|d| d.edges()) {
            //     if !node_sizes.contains_key(&data.source) {
            //         node_sizes.insert(data.source.clone(), egui::Vec2::new(42.0, 42.0));
            //     }
            //     if !node_sizes.contains_key(&data.target) {
            //         node_sizes.insert(data.target.clone(), egui::Vec2::new(42.0, 42.0));
            //     }
            // }

            let layout = compute_layout(
                node_sizes.into_iter(),
                edge_system
                    .data
                    .iter()
                    .flat_map(|d| d.edges().map(|e| (e.source, e.target))),
            );

            if let Some(bounding_box) = bounding_rect_from_iter(layout.values()) {
                state.world_to_view = fit_bounding_rect_to_screen(
                    bounding_box.scale_from_center(1.05),
                    clip_rect_window.size(),
                );
            }

            state.layout = Some(layout);

            return Ok(());
        };

        let response = ui.interact(clip_rect_window, id, egui::Sense::click_and_drag());

        // Allow dragging the background as well.
        if response.dragged() {
            state.world_to_view.translation += response.drag_delta();
        }

        let view_to_window = TSTransform::from_translation(ui.min_rect().left_top().to_vec2());
        let world_to_window = view_to_window * state.world_to_view;

        #[cfg(debug_assertions)]
        if response.double_clicked() {
            if let Some(window) = response.interact_pointer_pos() {
                log::debug!(
                    "Click event! Window: {:?}, View: {:?} World: {:?}",
                    window,
                    view_to_window.inverse() * window,
                    world_to_window.inverse() * window,
                );
            }
        }

        if let Some(pointer) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            // Note: doesn't catch zooming / panning if a button in this PanZoom container is hovered.
            if response.hovered() {
                let pointer_in_world = world_to_window.inverse() * pointer;
                let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
                let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);

                // Zoom in on pointer:
                state.world_to_view = state.world_to_view
                    * TSTransform::from_translation(pointer_in_world.to_vec2())
                    * TSTransform::from_scaling(zoom_delta)
                    * TSTransform::from_translation(-pointer_in_world.to_vec2());

                // Pan:
                state.world_to_view =
                    TSTransform::from_translation(pan_delta) * state.world_to_view;
            }
        }

        let clip_rect_world = world_to_window.inverse() * clip_rect_window;

        let window_layer = ui.layer_id();

        //#[cfg(debug_assertions)]
        #[cfg(any())]
        {
            let debug_id = egui::LayerId::new(egui::Order::Debug, id.with("debug_layer"));
            ui.ctx().set_transform_layer(debug_id, world_to_window);

            // Paint the coordinate system.
            let painter = egui::Painter::new(ui.ctx().clone(), debug_id, clip_rect_world);

            // paint coordinate system at the world origin
            let origin = egui::Pos2::new(0.0, 0.0);
            let x_axis = egui::Pos2::new(100.0, 0.0);
            let y_axis = egui::Pos2::new(0.0, 100.0);

            painter.line_segment([origin, x_axis], egui::Stroke::new(1.0, egui::Color32::RED));
            painter.line_segment(
                [origin, y_axis],
                egui::Stroke::new(2.0, egui::Color32::GREEN),
            );

            if let Some(bounding_box) = bounding_rect_from_iter(layout.values()) {
                painter.rect(
                    bounding_box,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(255, 0, 255, 32),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 0, 255)),
                );
            }
        }

        for data in node_system.data.iter() {
            let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());
            // We keep track of the size of the current entity.
            let mut entity_rect: Option<egui::Rect> = None;

            for node in data.nodes() {
                let current_extent = layout
                    .get(&node.location)
                    .expect("missing layout information for node");
                let response = egui::Area::new(id.with((node.location.clone(), node.instance)))
                    .current_pos(current_extent.min)
                    .order(egui::Order::Middle)
                    .constrain(false)
                    .show(ui.ctx(), |ui| {
                        let highlight = ent_highlight.index_highlight(node.instance);
                        ui.set_clip_rect(clip_rect_world);
                        node.draw(ui, highlight)
                    })
                    .response;

                let instance = InstancePath::instance(data.entity_path.clone(), node.instance);
                ctx.select_hovered_on_click(
                    &response,
                    Item::DataResult(query.space_view_id, instance),
                );

                layout.insert(node.location.clone(), response.rect);
                entity_rect =
                    entity_rect.map_or(Some(response.rect), |e| Some(e.union(response.rect)));

                let id = response.layer_id;
                ui.ctx().set_transform_layer(id, world_to_window);
                ui.ctx().set_sublayer(window_layer, id);
            }

            let entity_path = data.entity_path.clone();
            if let Some(entity_rect) = entity_rect {
                let entity_id = egui::LayerId::new(
                    egui::Order::Background,
                    id.with(("debug", entity_path.hash())),
                );
                ui.ctx().set_transform_layer(entity_id, world_to_window);
                let painter = egui::Painter::new(ui.ctx().clone(), entity_id, clip_rect_world);

                let padded = entity_rect.expand(10.0);
                let tc = ui.ctx().style().visuals.text_color();
                painter.rect(
                    padded,
                    ui.style().visuals.window_rounding,
                    egui::Color32::from_rgba_unmultiplied(tc.r(), tc.g(), tc.b(), 4),
                    egui::Stroke::NONE,
                );
                if (query
                    .highlights
                    .entity_outline_mask(entity_path.hash())
                    .overall
                    .is_some())
                {
                    // TODO(grtlr): text should be presented in window space.
                    painter.text(
                        padded.left_top(),
                        egui::Align2::LEFT_BOTTOM,
                        entity_path.to_string(),
                        egui::FontId::default(),
                        ui.ctx().style().visuals.text_color(),
                    );
                }
            }
        }

        let graph = Graph::new(
            node_system.data.iter().flat_map(|d| d.nodes()),
            edge_system.data.iter().flat_map(|d| d.edges()),
        );

        for dummy in graph.dummy_nodes() {
            let current_extent = layout
                .get(&dummy.0)
                .expect("missing layout information for dummy node");
            let response = egui::Area::new(id.with(dummy.0.clone()))
                .current_pos(current_extent.min)
                .order(egui::Order::Middle)
                .constrain(false)
                .show(ui.ctx(), |ui| {
                    ui.set_clip_rect(clip_rect_world);
                    draw_dummy(ui, dummy.1.clone(), dummy.0.node_id.clone())
                })
                .response;

            layout.insert(dummy.0.clone(), response.rect);

            let id = response.layer_id;
            ui.ctx().set_transform_layer(id, world_to_window);
            ui.ctx().set_sublayer(window_layer, id);
        }

        for data in edge_system.data.iter() {
            let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

            for EdgeInstance {
                source,
                target,
                instance,
                color,
                ..
            } in data.edges()
            {
                if let (Some(source_pos), Some(target_pos)) =
                    (layout.get(&source), layout.get(&target))
                {
                    let highlight = ent_highlight.index_highlight(instance);

                    let hcolor = match (
                        highlight.hover,
                        highlight.selection != SelectionHighlight::None,
                    ) {
                        (HoverHighlight::None, false) => None,
                        (HoverHighlight::None, true) => Some(ui.style().visuals.selection.bg_fill),
                        (HoverHighlight::Hovered, ..) => {
                            Some(ui.style().visuals.widgets.hovered.bg_fill)
                        }
                    };

                    let response = egui::Area::new(id.with((data.entity_path.hash(), instance)))
                        .current_pos(source_pos.center())
                        .order(egui::Order::Background)
                        .constrain(false)
                        .show(ui.ctx(), |ui| {
                            ui.set_clip_rect(world_to_window.inverse() * clip_rect_window);
                            egui::Frame::default().show(ui, |ui| {
                                let painter = ui.painter();
                                if let Some(hcolor) = hcolor {
                                    painter.line_segment(
                                        [source_pos.center(), target_pos.center()],
                                        egui::Stroke::new(4.0, hcolor),
                                    );
                                }
                                painter.line_segment(
                                    [source_pos.center(), target_pos.center()],
                                    egui::Stroke::new(
                                        1.0,
                                        color.unwrap_or(ui.style().visuals.text_color()),
                                    ),
                                );
                            });
                        })
                        .response;

                    let id = response.layer_id;

                    ui.ctx().set_transform_layer(id, world_to_window);
                    ui.ctx().set_sublayer(window_layer, id);
                } else {
                    log::warn!(
                        "Missing layout information for edge: {:?} -> {:?}",
                        source,
                        target
                    );
                }
            }
        }

        Ok(())
    }
}
