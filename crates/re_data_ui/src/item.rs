use re_viewer_context::{Item, UiVerbosity, ViewerContext};

use crate::DataUi;

impl DataUi for Item {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Item::SpaceView(_) | Item::DataBlueprintGroup(_, _) => {
                // Shouldn't be reachable since SelectionPanel::contents doesn't show data ui for these.
                // If you add something in here make sure to adjust SelectionPanel::contents accordingly.
            }
            Item::ComponentPath(component_path) => {
                component_path.data_ui(ctx, ui, verbosity, query);
            }
            Item::InstancePath(_, instance_path) => {
                instance_path.data_ui(ctx, ui, verbosity, query);
            }
        }
    }
}
