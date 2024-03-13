use crate::{
    SpaceViewClass, SpaceViewClassIdentifier, SpaceViewClassRegistryError,
    SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewSystemExecutionError,
    SpaceViewSystemRegistrator, SystemExecutionOutput, ViewQuery, ViewerContext,
};
use re_entity_db::EntityProperties;

/// A placeholder space view class that can be used when the actual class is not registered.
#[derive(Default)]
pub struct SpaceViewClassPlaceholder;

impl SpaceViewClass for SpaceViewClassPlaceholder {
    fn identifier() -> SpaceViewClassIdentifier {
        "Unknown Space View Class".into()
    }

    fn display_name(&self) -> &'static str {
        "Unknown Space View Class"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_UNKNOWN
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "The Space View Class was not recognized.\nThis happens if either the Blueprint specifies an invalid Space View Class or this version of the Viewer does not know about this type.".into()
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
        SpaceViewSpawnHeuristics {
            recommended_space_views: Vec::new(),
        }
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut dyn SpaceViewState,
        _root_entity_properties: &EntityProperties,
        _query: &ViewQuery<'_>,
        _system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        ui.centered_and_justified(|ui| ui.label(self.help_text(ctx.re_ui)));
        Ok(())
    }
}
