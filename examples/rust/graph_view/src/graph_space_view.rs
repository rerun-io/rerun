use re_viewer::external::{
    egui::{self, Label},
    re_data_ui::{item_ui, DataUi},
    re_entity_db::InstancePath,
    re_log_types::EntityPath,
    re_types::SpaceViewClassIdentifier,
    re_ui,
    re_viewer_context::{
        HoverHighlight, IdentifiedViewSystem as _, Item, SelectionHighlight, SpaceViewClass,
        SpaceViewClassLayoutPriority, SpaceViewClassRegistryError, SpaceViewId,
        SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt as _,
        SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, UiLayout,
        ViewQuery, ViewerContext,
    },
};

use crate::graph_visualizer_system::{GraphNodeSystem, NodeIdWithInstance};

// /// The different modes for displaying color coordinates in the custom space view.
// #[derive(Default, Debug, PartialEq, Clone, Copy)]
// enum ColorCoordinatesMode {
//     #[default]
//     Hs,
//     Hv,
//     Rg,
// }

// impl ColorCoordinatesMode {
//     pub const ALL: [ColorCoordinatesMode; 3] = [
//         ColorCoordinatesMode::Hs,
//         ColorCoordinatesMode::Hv,
//         ColorCoordinatesMode::Rg,
//     ];
// }

// impl std::fmt::Display for ColorCoordinatesMode {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             ColorCoordinatesMode::Hs => "Hue/Saturation".fmt(f),
//             ColorCoordinatesMode::Hv => "Hue/Value".fmt(f),
//             ColorCoordinatesMode::Rg => "Red/Green".fmt(f),
//         }
//     }
// }

/// Space view state for the custom space view.
///
/// This state is preserved between frames, but not across Viewer sessions.
#[derive(Default)]
pub struct GraphSpaceViewState {
    // TODO(wumpf, jleibs): This should be part of the Blueprint so that it is serialized out.
    //                      but right now there is no way of doing that.
    // mode: ColorCoordinatesMode,
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
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

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
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,

        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let graph_nodes = system_output.view_systems.get::<GraphNodeSystem>()?;
        let state = state.downcast_mut::<GraphSpaceViewState>()?;

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
                            let text = egui::RichText::new(format!("{:?}", n.node_id.0.0));
                            ui.add(Label::new(text));
                        }
                    }
                })
            })
            .response
        });
        Ok(())
    }
}
