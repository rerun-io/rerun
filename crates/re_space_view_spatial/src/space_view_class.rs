use re_log_types::EntityPath;
use re_viewer_context::{SpaceViewClassImpl, SpaceViewId};

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
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::SpaceViewState,
    ) {
        state.selection_ui(ctx, ui);
    }

    fn ui(
        &self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::SpaceViewState,
        scene: &mut re_viewer_context::TypedScene<Self>,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        state.view_spatial(ctx, ui, scene, space_origin, space_view_id);
    }
}
