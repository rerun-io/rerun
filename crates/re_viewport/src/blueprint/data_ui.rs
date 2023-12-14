use re_data_ui::{DataUi, EntityDataUi};
use re_viewer_context::{ComponentUiRegistry, SpaceViewId, UiVerbosity, ViewerContext};

use super::components::{IncludedSpaceViews, SpaceViewMaximized, ViewportLayout};

impl DataUi for IncludedSpaceViews {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label(format!("{} Space Views", self.0.len()));
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight | UiVerbosity::Reduced => {
                for space_view in &self.0 {
                    let space_view: SpaceViewId = (*space_view).into();
                    space_view.data_ui(_ctx, ui, verbosity, _query);
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
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self.0 {
            Some(space_view) => {
                let space_view: SpaceViewId = space_view.into();
                space_view.data_ui(ctx, ui, verbosity, query);
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
        _query: &re_arrow_store::LatestAtQuery,
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

    /// Registers how to show a given component in the ui.
    pub fn add<C: EntityDataUi + re_types::Component>(registry: &mut ComponentUiRegistry) {
        registry.add(
            C::name(),
            Box::new(
                |ctx, ui, verbosity, query, entity_path, component, instance| match component
                    .lookup::<C>(instance)
                {
                    Ok(component) => {
                        component.entity_data_ui(ctx, ui, verbosity, entity_path, query);
                    }
                    Err(re_query::QueryError::ComponentNotFound) => {
                        ui.weak("(not found)");
                    }
                    Err(err) => {
                        re_log::warn_once!("Expected component {}, {}", C::name(), err);
                    }
                },
            ),
        );
    }

    add::<IncludedSpaceViews>(registry);
    add::<SpaceViewMaximized>(registry);
    add::<ViewportLayout>(registry);
}
