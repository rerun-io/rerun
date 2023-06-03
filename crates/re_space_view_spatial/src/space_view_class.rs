use re_space_view::EmptySpaceViewState;
use re_viewer_context::SpaceViewClassImpl;

use crate::scene::{SpatialSceneContext, SpatialScenePartCollection};

pub struct SpatialSpaceViewClass;

impl SpaceViewClassImpl for SpatialSpaceViewClass {
    type SpaceViewState = EmptySpaceViewState;
    type SceneContext = SpatialSceneContext;
    type ScenePartCollection = SpatialScenePartCollection;

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
        //todo!()
    }

    fn ui(
        &self,
        _ctx: &mut re_viewer_context::ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut Self::SpaceViewState,
        _scene: &re_viewer_context::TypedScene<Self>,
    ) {
        // TODO(andreas)
        //todo!()
    }
}
