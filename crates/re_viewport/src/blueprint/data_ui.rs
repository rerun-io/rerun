use re_data_ui::{add_to_registry, DataUi};
use re_viewer_context::{ComponentUiRegistry, SpaceViewId, UiVerbosity, ViewerContext};

use super::components::{IncludedSpaceView, SpaceViewMaximized};

impl DataUi for IncludedSpaceView {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        let space_view: SpaceViewId = self.0.into();
        space_view.data_ui(ctx, ui, verbosity, query, store);
    }
}

impl DataUi for SpaceViewMaximized {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        let space_view: SpaceViewId = self.0.into();
        space_view.data_ui(ctx, ui, verbosity, query, store);
    }
}

pub fn register_ui_components(registry: &mut ComponentUiRegistry) {
    re_tracing::profile_function!();

    add_to_registry::<IncludedSpaceView>(registry);
    add_to_registry::<SpaceViewMaximized>(registry);
}
