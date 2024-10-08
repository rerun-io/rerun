use std::collections::HashMap;

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
    common::NodeLocation, edge_undirected_visualizer_system::EdgeUndirectedVisualizer, error::Error, graph::{Graph, Node}, ui
};
use crate::{
    edge_undirected_visualizer_system::{self, EdgeInstance},
    node_visualizer_system::{GraphNodeVisualizer, NodeInstance},
};

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

            let node_sizes = ui::measure_node_sizes(ui, graph.nodes());

            let layout = crate::layout::compute_layout(
                node_sizes.into_iter(),
                edge_system
                    .data
                    .iter()
                    .flat_map(|d| d.edges().map(|e| (e.source, e.target))),
            )?;

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
                        ui::draw_node(ui, &node, highlight)
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
                ui::draw_entity(
                    ui,
                    clip_rect_world,
                    entity_id,
                    entity_rect,
                    &entity_path,
                    &query.highlights,
                );
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
                    ui::draw_dummy(ui, dummy.1, &dummy.0.node_id)
                })
                .response;

            layout.insert(dummy.0.clone(), response.rect);

            let id = response.layer_id;
            ui.ctx().set_transform_layer(id, world_to_window);
            ui.ctx().set_sublayer(window_layer, id);
        }

        for data in edge_system.data.iter() {
            let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

            for edge in data.edges() {
                let source_pos = layout
                    .get(&edge.source)
                    .ok_or_else(|| Error::EdgeUnknownNode(edge.source.to_string()))?;
                let target_pos = layout
                    .get(&edge.target)
                    .ok_or_else(|| Error::EdgeUnknownNode(edge.target.to_string()))?;

                let response = egui::Area::new(id.with((data.entity_path.hash(), edge.instance)))
                    .current_pos(source_pos.center())
                    .order(egui::Order::Background)
                    .constrain(false)
                    .show(ui.ctx(), |ui| {
                        ui.set_clip_rect(world_to_window.inverse() * clip_rect_window);
                        ui::draw_edge(
                            ui,
                            edge.color,
                            source_pos,
                            target_pos,
                            ent_highlight.index_highlight(edge.instance),
                        );
                    })
                    .response;

                let id = response.layer_id;

                ui.ctx().set_transform_layer(id, world_to_window);
                ui.ctx().set_sublayer(window_layer, id);
            }
        }

        Ok(())
    }
}
