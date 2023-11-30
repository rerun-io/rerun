use crate::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewSystemExecutionError,
    SpaceViewSystemRegistry, ViewContextCollection, ViewPartCollection, ViewQuery, ViewerContext,
};
use re_data_store::EntityProperties;

/// A placeholder space view class that can be used when the actual class is not registered.
#[derive(Default)]
pub struct SpaceViewClassPlaceholder;

impl SpaceViewClass for SpaceViewClassPlaceholder {
    type State = ();

    const NAME: &'static str = "Unknown Space View Class";
    const DISPLAY_NAME: &'static str = "Unknown Space View Class";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_UNKNOWN
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "The Space View Class was not recognized.\nThis happens if either the Blueprint specifies an invalid Space View Class or this version of the Viewer does not know about this type.".into()
    }

    fn on_register(
        &self,
        _system_registry: &mut SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        Ok(())
    }

    fn layout_priority(&self) -> crate::SpaceViewClassLayoutPriority {
        crate::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        _ctx: &mut crate::ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut (),
        _space_origin: &re_log_types::EntityPath,
        _space_view_id: crate::SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
    }

    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        _view_ctx: &ViewContextCollection,
        _parts: &ViewPartCollection,
        _query: &ViewQuery<'_>,
        _draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        ui.centered_and_justified(|ui| ui.label(self.help_text(ctx.re_ui)));
        Ok(())
    }
}
