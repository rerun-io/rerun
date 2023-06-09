use crate::{DynSpaceViewClass, Scene, SpaceViewClassName};

/// A placeholder space view class that can be used when the actual class is not registered.
#[derive(Default)]
pub struct SpaceViewClassPlaceholder;

impl DynSpaceViewClass for SpaceViewClassPlaceholder {
    fn name(&self) -> SpaceViewClassName {
        "Unknown Space View Class".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_UNKNOWN
    }

    fn help_text(
        &self,
        _re_ui: &re_ui::ReUi,
        _state: &dyn crate::SpaceViewState,
    ) -> egui::WidgetText {
        "The Space View Class was not recognized.\nThis happens if either the Blueprint specifies an invalid Space View Class or this version of the Viewer does not know about this type.".into()
    }

    fn new_state(&self) -> Box<dyn crate::SpaceViewState> {
        Box::new(())
    }

    fn new_scene(&self) -> Box<dyn crate::Scene> {
        Box::<EmptyScene>::default()
    }

    fn blueprint_archetype(&self) -> Option<crate::ArchetypeDefinition> {
        None
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn crate::SpaceViewState) -> Option<f32> {
        None
    }

    fn prepare_populate(
        &self,
        _ctx: &mut crate::ViewerContext<'_>,
        _state: &mut dyn crate::SpaceViewState,
        _entity_paths: &nohash_hasher::IntSet<re_log_types::EntityPath>,
        _entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
    }

    fn selection_ui(
        &self,
        _ctx: &mut crate::ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut dyn crate::SpaceViewState,
        _space_origin: &re_log_types::EntityPath,
        _space_view_id: crate::SpaceViewId,
    ) {
    }

    fn ui(
        &self,
        ctx: &mut crate::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn crate::SpaceViewState,
        _scene: Box<dyn crate::Scene>,
        _space_origin: &re_log_types::EntityPath,
        _space_view_id: crate::SpaceViewId,
    ) {
        ui.centered_and_justified(|ui| ui.label(self.help_text(ctx.re_ui, state)));
    }
}

#[derive(Default)]
struct EmptyScene;

impl Scene for EmptyScene {
    fn populate(
        &mut self,
        _ctx: &mut crate::ViewerContext<'_>,
        _query: &crate::SceneQuery<'_>,
        _space_view_state: &dyn crate::SpaceViewState,
        _highlights: crate::SpaceViewHighlights,
    ) {
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
