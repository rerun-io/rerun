use re_viewer::external::{
    egui::{self, emath::TSTransform},
    re_entity_db::InstancePath,
    re_log::external::log,
    re_log_types::EntityPath,
    re_types::SpaceViewClassIdentifier,
    re_ui::{self, UiExt},
    re_viewer_context::{
        IdentifiedViewSystem as _, Item, SpaceViewClass, SpaceViewClassLayoutPriority,
        SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewState,
        SpaceViewStateExt as _, SpaceViewSystemExecutionError, SpaceViewSystemRegistrator,
        SystemExecutionOutput, ViewQuery, ViewerContext,
    },
};

use crate::{
    error::Error,
    graph::{Graph, NodeIndex},
    ui::{self, GraphSpaceViewState},
    visualizers::{EdgesDirectedVisualizer, EdgesUndirectedVisualizer, NodeVisualizer},
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
        system_registry.register_visualizer::<NodeVisualizer>()?;
        system_registry.register_visualizer::<EdgesDirectedVisualizer>()?;
        system_registry.register_visualizer::<EdgesUndirectedVisualizer>()
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
            .get(&NodeVisualizer::identifier())
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
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        ui.selection_grid("graph_settings_ui").show(ui, |ui| {
            state.bounding_box_ui(ui);
            state.debug_ui(ui);
            state.layout_provider_ui(ui);
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
        let node_system = system_output.view_systems.get::<NodeVisualizer>()?;
        let directed_system = system_output
            .view_systems
            .get::<EdgesDirectedVisualizer>()?;
        let undirected_system = system_output
            .view_systems
            .get::<EdgesUndirectedVisualizer>()?;

        let Some(graph) = Graph::from_nodes_edges(
            &node_system.data,
            &directed_system.data,
            &undirected_system.data,
        ) else {
            log::warn!("No graph data available.");
            return Ok(());
        };

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        let Some(layout) = &mut state.layout else {
            let node_sizes = ui::measure_node_sizes(ui, graph.all_nodes());

            let undirected = undirected_system
                .data
                .iter()
                .flat_map(|d| d.edges().map(|e| (e.source.into(), e.target.into())));

            let directed = directed_system
                .data
                .iter()
                .flat_map(|d| d.edges().map(|e| (e.source.into(), e.target.into())));

            let layout =
                state
                    .layout_provider
                    .compute(node_sizes.into_iter(), undirected, directed)?;

            // if let Some(bounding_box) = ui::bounding_rect_from_iter(layout.values()) {
            //     state
            //         .viewer
            //         .fit_to_screen(bounding_box.scale_from_center(1.05));
            // }

            state.layout = Some(layout);

            return Ok(());
        };

        if graph
            .all_nodes()
            .any(|n| !layout.contains_key(&NodeIndex::from(&n)))
        {
            state.layout = None;
            return Ok(());
        }

        state.viewer.scene(ui, |mut scene| {
            for data in node_system.data.iter() {
                let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

                // We keep track of the size of the current entity.
                // TODO: let mut entity_rect: Option<egui::Rect> = None;

                for node in data.nodes() {
                    let index = NodeIndex::from(&node);
                    let current = layout.get(&index).unwrap();

                    let response = scene.node(current.min, |ui| {
                        ui::draw_node(ui, &node, ent_highlight.index_highlight(node.instance))
                    });

                    let instance = InstancePath::instance(data.entity_path.clone(), node.instance);
                    ctx.select_hovered_on_click(
                        &response,
                        Item::DataResult(query.space_view_id, instance),
                    );

                    layout.insert(index, response.rect);
                    // entity_rect =
                    //     entity_rect.map_or(Some(response.rect), |e| Some(e.union(response.rect)));
                }
            }
        });

        //     let entity_path = data.entity_path.clone();

        //     if let Some(entity_rect) = entity_rect {
        //         let entity_id = egui::LayerId::new(
        //             egui::Order::Background,
        //             id.with(("debug", entity_path.hash())),
        //         );
        //         ui.ctx().set_transform_layer(entity_id, world_to_window);
        //         ui::draw_entity(
        //             ui,
        //             clip_rect_world,
        //             entity_id,
        //             entity_rect,
        //             &entity_path,
        //             &query.highlights,
        //         );
        //     }
        // }

        // for dummy in graph.unknown_nodes() {
        //     let index = NodeIndex::from(&dummy);
        //     let current_extent = layout
        //         .get(&index)
        //         .expect("missing layout information for dummy node");
        //     let response = egui::Area::new(id.with(&index))
        //         .current_pos(current_extent.min)
        //         .order(egui::Order::Middle)
        //         .constrain(false)
        //         .show(ui.ctx(), |ui| {
        //             ui.set_clip_rect(clip_rect_world);
        //             ui::draw_dummy(ui, &dummy)
        //         })
        //         .response;

        //     layout.insert(index, response.rect);

        //     let id = response.layer_id;
        //     ui.ctx().set_transform_layer(id, world_to_window);
        //     ui.ctx().set_sublayer(window_layer, id);
        // }

        // for data in undirected_system.data.iter() {
        //     let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

        //     for edge in data.edges() {
        //         let source_ix = NodeIndex::from(edge.source);
        //         let target_ix = NodeIndex::from(edge.target);
        //         let source_pos = layout.get(&source_ix).ok_or(Error::EdgeUnknownNode)?;
        //         let target_pos = layout.get(&target_ix).ok_or(Error::EdgeUnknownNode)?;

        //         let response = egui::Area::new(id.with((data.entity_path.hash(), edge.instance)))
        //             .current_pos(source_pos.center())
        //             .order(egui::Order::Background)
        //             .constrain(false)
        //             .show(ui.ctx(), |ui| {
        //                 ui.set_clip_rect(world_to_window.inverse() * clip_rect_window);
        //                 ui::draw_edge(
        //                     ui,
        //                     edge.color,
        //                     source_pos,
        //                     target_pos,
        //                     ent_highlight.index_highlight(edge.instance),
        //                     false,
        //                 );
        //             })
        //             .response;

        //         let id = response.layer_id;

        //         ui.ctx().set_transform_layer(id, world_to_window);
        //         ui.ctx().set_sublayer(window_layer, id);
        //     }
        // }

        // for data in directed_system.data.iter() {
        //     let ent_highlight = query.highlights.entity_highlight(data.entity_path.hash());

        //     for edge in data.edges() {
        //         let source_ix = NodeIndex::from(edge.source);
        //         let target_ix = NodeIndex::from(edge.target);
        //         let source_pos = layout.get(&source_ix).ok_or(Error::EdgeUnknownNode)?;
        //         let target_pos = layout.get(&target_ix).ok_or(Error::EdgeUnknownNode)?;

        //         let response = egui::Area::new(id.with((data.entity_path.hash(), edge.instance)))
        //             .current_pos(source_pos.center())
        //             .order(egui::Order::Background)
        //             .constrain(false)
        //             .show(ui.ctx(), |ui| {
        //                 ui.set_clip_rect(world_to_window.inverse() * clip_rect_window);
        //                 ui::draw_edge(
        //                     ui,
        //                     edge.color,
        //                     source_pos,
        //                     target_pos,
        //                     ent_highlight.index_highlight(edge.instance),
        //                     true,
        //                 );
        //             })
        //             .response;

        //         let id = response.layer_id;

        //         ui.ctx().set_transform_layer(id, world_to_window);
        //         ui.ctx().set_sublayer(window_layer, id);
        //     }
        // }

        Ok(())
    }
}
