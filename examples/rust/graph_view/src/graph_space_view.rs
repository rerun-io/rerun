use re_viewer::external::{
    egui::{self, Label},
    re_log::external::log,
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

use crate::edge_visualizer_system::GraphEdgeSystem;
use crate::node_visualizer_system::GraphNodeVisualizer;

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct GraphSpaceViewState;

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
        system_registry.register_visualizer::<GraphEdgeSystem>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        log::debug!("Creating new GraphSpaceViewState");
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
        let node_system = system_output.view_systems.get::<GraphNodeVisualizer>()?;
        let edge_system = system_output.view_systems.get::<GraphEdgeSystem>()?;

        let state = state.downcast_mut::<GraphSpaceViewState>()?;

        egui::Frame {
            inner_margin: re_ui::DesignTokens::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.label(egui::RichText::new("Nodes").underline());

                    for data in node_system.data.iter() {
                        for (node_id, maybe_color) in data.nodes.iter() {
                            let text = egui::RichText::new(format!(
                                "{}: {}",
                                data.entity_path.to_owned(),
                                node_id.0
                            ));

                            if let Some(color) = maybe_color {
                                let c = egui::Color32::from(color.0);
                                ui.add(Label::new(
                                    text.color(c)),
                                );
                            } else {
                                ui.add(Label::new(text));
                            }
                        }
                    }

                    ui.label(egui::RichText::new("Edges").underline());

                    for (entity, edges) in edge_system.edges.iter() {
                        for e in edges {
                            let text = egui::RichText::new(format!(
                                "{}: {:?}:{} -> {:?}:{}",
                                entity.to_owned(),
                                e.edge.0.source_entity.clone().map(EntityPath::from),
                                e.edge.0.source,
                                e.edge.0.target_entity.clone().map(EntityPath::from),
                                e.edge.0.target
                            ));
                            ui.add(Label::new(text));
                        }
                    }
                })
            });
        });

        Ok(())
    }
}
