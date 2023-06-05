use re_viewer_context::SpaceViewClassImpl;

use crate::{
    scene::{SpatialSceneContext, SpatialScenePartCollection, SpatialScenePartData},
    SpatialSpaceViewState,
};

pub struct SpatialSpaceViewClass;

impl SpaceViewClassImpl for SpatialSpaceViewClass {
    type SpaceViewState = SpatialSpaceViewState;
    type SceneContext = SpatialSceneContext;
    type ScenePartCollection = SpatialScenePartCollection;
    type ScenePartData = SpatialScenePartData;

    fn name(&self) -> re_viewer_context::SpaceViewClassName {
        "Spatial".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_3D
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        // TODO(andreas)
        "todo".into()
    }

    fn selection_ui(
        &self,
        _ctx: &mut re_viewer_context::ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut Self::SpaceViewState,
    ) {
        // TODO(andreas)
    }

    fn ui(
        &self,
        _ctx: &mut re_viewer_context::ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut Self::SpaceViewState,
        _scene: &mut re_viewer_context::TypedScene<Self>,
    ) {
        // TODO(andreas)
    }
}
