use re_log_types::EntityPath;
use re_viewer_context::{SpaceViewClassImpl, SpaceViewId};

use crate::{
    scene::{SpatialSceneContext, SpatialScenePartCollection, SpatialScenePartData},
    ui::{SpatialNavigationMode, SpatialSpaceViewState},
};

#[derive(Default)]
pub struct SpatialSpaceView;

impl SpaceViewClassImpl for SpatialSpaceView {
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
        // TODO:
        "todo".into()
    }

    fn preferred_tile_aspect_ratio(&self, state: &Self::SpaceViewState) -> Option<f32> {
        match state.nav_mode.get() {
            SpatialNavigationMode::TwoD => {
                let size = state.scene_bbox_accum.size();
                Some(size.x / size.y)
            }
            SpatialNavigationMode::ThreeD => None,
        }
    }

    fn prepare_populate(
        &self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        state: &Self::SpaceViewState,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
        state.update_object_property_heuristics(ctx, entity_properties);
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
