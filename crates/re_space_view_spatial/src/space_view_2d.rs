use nohash_hasher::IntSet;
use re_log_types::EntityPath;
use re_viewer_context::{
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartCollection, ViewQuery, ViewerContext,
};

use crate::{
    contexts::register_contexts,
    parts::register_parts,
    ui::{SpatialNavigationMode, SpatialSpaceViewState},
};

#[derive(Default)]
pub struct SpatialSpaceView2D;

impl SpaceViewClass for SpatialSpaceView2D {
    type State = SpatialSpaceViewState;

    fn name(&self) -> re_viewer_context::SpaceViewClassName {
        "2D".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_2D
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        super::ui_2d::help_text(re_ui)
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        register_contexts(system_registry)?;
        register_parts(system_registry)?;
        Ok(())
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

    fn prepare_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
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
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        view_ctx: &ViewContextCollection,
        parts: &ViewPartCollection,
        query: &ViewQuery<'_>,
        draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        state.view_spatial(ctx, ui, view_ctx, parts, query, draw_data)
    }
}
