use nohash_hasher::IntSet;
use re_log_types::EntityPath;
use re_viewer_context::{SpaceViewClass, SpaceViewId};

use crate::{
    scene::SpatialScenePartCollection,
    ui::{SpatialNavigationMode, SpatialSpaceViewState},
};

#[derive(Default)]
pub struct SpatialSpaceView;

impl SpaceViewClass for SpatialSpaceView {
    type State = SpatialSpaceViewState;
    type SceneParts = SpatialScenePartCollection;

    fn name(&self) -> re_viewer_context::SpaceViewClassName {
        "Spatial".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_3D
    }

    fn help_text(&self, re_ui: &re_ui::ReUi, state: &Self::State) -> egui::WidgetText {
        state.help_text(re_ui)
    }

    fn preferred_tile_aspect_ratio(&self, state: &Self::State) -> Option<f32> {
        match state.nav_mode.get() {
            SpatialNavigationMode::TwoD => {
                let size = state.scene_bbox_accum.size();
                Some(size.x / size.y)
            }
            SpatialNavigationMode::ThreeD => None,
        }
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
    }

    fn prepare_populate(
        &self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        state: &Self::State,
        entity_paths: &IntSet<EntityPath>,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
        state.update_object_property_heuristics(ctx, entity_paths, entity_properties);
    }

    fn selection_ui(
        &self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        state.selection_ui(ctx, ui, space_origin, space_view_id);
    }

    fn ui(
        &self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        scene: &mut re_viewer_context::TypedScene<Self>,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        state.view_spatial(ctx, ui, scene, space_origin, space_view_id);
    }
}
