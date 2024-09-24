use egui_graphs::SettingsInteraction;
use re_viewer::external::{
    egui::{self, Label, Stroke},
    re_log_types::EntityPath,
    re_types::SpaceViewClassIdentifier,
    re_ui,
    re_viewer_context::{
        IdentifiedViewSystem as _, SpaceViewClass, SpaceViewClassLayoutPriority,
        SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewState,
        SpaceViewStateExt as _, SpaceViewSystemExecutionError, SpaceViewSystemRegistrator,
        SystemExecutionOutput, ViewQuery, ViewerContext,
    },
};

use crate::graph_visualizer_system::GraphNodeSystem;

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
pub struct GraphSpaceViewState {
    graph: egui_graphs::Graph<(), ()>,
}

impl Default for GraphSpaceViewState {
    fn default() -> Self {
        let mut g = petgraph::stable_graph::StableGraph::new();

        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());

        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.add_edge(c, a, ());

        Self {
            graph: egui_graphs::Graph::from(&g),
        }
    }
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
        system_registry.register_visualizer::<GraphNodeSystem>()
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
            .get(&GraphNodeSystem::identifier())
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
        _ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let _state = state.downcast_mut::<GraphSpaceViewState>()?;

        // ui.horizontal(|ui| {
        //     ui.label("Coordinates mode");
        //     egui::ComboBox::from_id_salt("color_coordinates_mode")
        //         .selected_text(state.mode.to_string())
        //         .show_ui(ui, |ui| {
        //             for mode in &ColorCoordinatesMode::ALL {
        //                 ui.selectable_value(&mut state.mode, *mode, mode.to_string());
        //             }
        //         });
        // });

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
        _query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let graph_nodes = system_output.view_systems.get::<GraphNodeSystem>()?;
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        let interaction_settings = &SettingsInteraction::new()
            .with_dragging_enabled(true)
            .with_node_clicking_enabled(true)
            .with_node_selection_enabled(true)
            .with_node_selection_multi_enabled(true)
            .with_edge_clicking_enabled(true)
            .with_edge_selection_enabled(true)
            .with_edge_selection_multi_enabled(true);

        let navigation_settings =
            &egui_graphs::SettingsNavigation::new().with_fit_to_screen_enabled(true);

        let mut graph_view = egui_graphs::GraphView::new(&mut state.graph)
            .with_interactions(interaction_settings)
            .with_navigations(navigation_settings);

        egui::Frame {
            inner_margin: re_ui::DesignTokens::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    for (entity, nodes) in graph_nodes.nodes.iter() {
                        let text = egui::RichText::new(entity.to_owned());
                        ui.add(Label::new(text));
                        for n in nodes {
                            let text = egui::RichText::new(format!("{:?}", n.node_id.0 .0));
                            ui.add(Label::new(text));
                        }
                    }
                })
            });

            egui::Frame::none()
                .stroke(Stroke {
                    width: 1.0,
                    color: egui::Color32::RED,
                })
                .show(ui, |ui| {
                    ui.add(&mut graph_view);
                });
        });

        Ok(())
    }
}
