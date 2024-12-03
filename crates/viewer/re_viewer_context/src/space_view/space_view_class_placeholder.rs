use crate::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewSpawnHeuristics, SpaceViewState,
    SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, ViewQuery,
    ViewerContext,
};
use re_types::SpaceViewClassIdentifier;
use re_ui::UiExt;

/// A placeholder space view class that can be used when the actual class is not registered.
#[derive(Default)]
pub struct SpaceViewClassPlaceholder;

impl SpaceViewClass for SpaceViewClassPlaceholder {
    fn identifier() -> SpaceViewClassIdentifier {
        "UnknownSpaceViewClass".into()
    }

    fn display_name(&self) -> &'static str {
        "Unknown space view class"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_UNKNOWN
    }

    fn help_markdown(&self, _egui_ctx: &egui::Context) -> String {
        "Placeholder view for unknown space view class".to_owned()
    }

    fn on_register(
        &self,
        _system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        Ok(())
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<()>::default()
    }

    fn layout_priority(&self) -> crate::SpaceViewClassLayoutPriority {
        crate::SpaceViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(&self, _ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        SpaceViewSpawnHeuristics::empty()
    }

    fn ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn SpaceViewState,
        _query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        egui::Frame {
            inner_margin: egui::Margin::same(re_ui::DesignTokens::view_padding()),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.warning_label("Unknown space view class");

            ui.markdown_ui(
                "This happens if either the blueprint specifies an invalid space view class or \
                this version of the viewer does not know about this type.\n\n\
                \
                **Note**: some views may require a specific Cargo feature to be enabled. In \
                particular, the map view requires the `map_view` feature.",
            );
        });

        Ok(())
    }
}

crate::impl_component_fallback_provider!(SpaceViewClassPlaceholder => []);
