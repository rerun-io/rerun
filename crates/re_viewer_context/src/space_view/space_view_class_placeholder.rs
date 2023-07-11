use crate::{ScenePartCollection, SpaceViewClass, SpaceViewClassName, ViewPartSystem};

/// A placeholder space view class that can be used when the actual class is not registered.
#[derive(Default)]
pub struct SpaceViewClassPlaceholder;

impl SpaceViewClass for SpaceViewClassPlaceholder {
    type State = ();
    type Context = ();
    type SceneParts = ();
    type ScenePartData = ();

    fn name(&self) -> SpaceViewClassName {
        "Unknown Space View Class".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_UNKNOWN
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi, _state: &()) -> egui::WidgetText {
        "The Space View Class was not recognized.\nThis happens if either the Blueprint specifies an invalid Space View Class or this version of the Viewer does not know about this type.".into()
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
    ) {
    }

    fn ui(
        &self,
        ctx: &mut crate::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut (),
        _scene: &mut crate::TypedScene<Self>,
        _space_origin: &re_log_types::EntityPath,
        _space_view_id: crate::SpaceViewId,
    ) {
        ui.centered_and_justified(|ui| ui.label(self.help_text(ctx.re_ui, state)));
    }
}

impl ScenePartCollection<SpaceViewClassPlaceholder> for () {
    fn vec_mut(&mut self) -> Vec<&mut dyn ViewPartSystem<SpaceViewClassPlaceholder>> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
