use re_data_ui::{add_to_registry, DataUi};
use re_viewer_context::{ComponentUiRegistry, SpaceViewId, UiVerbosity, ViewerContext};

use super::components::{IncludedSpaceViews, SpaceViewMaximized, ViewportLayout};

impl DataUi for IncludedSpaceViews {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_data_store::LatestAtQuery,
        store: &re_data_store::DataStore,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label(format!("{} Space Views", self.0.len()));
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight | UiVerbosity::Reduced => {
                for space_view in &self.0 {
                    let space_view: SpaceViewId = (*space_view).into();
                    space_view.data_ui(_ctx, ui, verbosity, query, store);
                    ui.end_row();
                }
            }
        }
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
        match self.0 {
            Some(space_view) => {
                let space_view: SpaceViewId = space_view.into();
                space_view.data_ui(ctx, ui, verbosity, query, store);
            }
            None => {
                ui.label("None");
            }
        }
    }
}

impl DataUi for ViewportLayout {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label(format!("ViewportLayout with {} tiles", self.0.tiles.len()));
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight | UiVerbosity::Reduced => {
                ui.label(format!("{:?}", self.0));
            }
        }
    }
}

pub fn register_ui_components(registry: &mut ComponentUiRegistry) {
    re_tracing::profile_function!();

    add_to_registry::<IncludedSpaceViews>(registry);
    add_to_registry::<SpaceViewMaximized>(registry);
    add_to_registry::<ViewportLayout>(registry);
}
