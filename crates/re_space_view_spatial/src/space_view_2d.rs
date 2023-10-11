use re_log_types::EntityPath;
use re_viewer_context::{
    AutoSpawnHeuristic, PerSystemEntities, SpaceViewClass, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartCollection,
    ViewQuery, ViewerContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{auto_spawn_heuristic, update_object_property_heuristics},
    parts::{calculate_bounding_box, register_2d_spatial_parts},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
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
        register_spatial_contexts(system_registry)?;
        register_2d_spatial_parts(system_registry)?;

        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, state: &Self::State) -> Option<f32> {
        let size = state.scene_bbox_accum.size();
        Some(size.x / size.y)
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
    }

    fn on_frame_start(
        &self,
        ctx: &mut ViewerContext<'_>,
        state: &Self::State,
        ent_paths: &PerSystemEntities,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
        update_object_property_heuristics(
            ctx,
            ent_paths,
            entity_properties,
            &state.scene_bbox_accum,
            SpatialSpaceViewKind::TwoD,
        );
    }

    fn auto_spawn_heuristic(
        &self,
        ctx: &ViewerContext<'_>,
        _space_origin: &EntityPath,
        ent_paths: &PerSystemEntities,
    ) -> AutoSpawnHeuristic {
        auto_spawn_heuristic(&self.name(), ctx, ent_paths, SpatialSpaceViewKind::TwoD)
    }

    fn selection_ui(
        &self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        state.selection_ui(
            ctx,
            ui,
            space_origin,
            space_view_id,
            SpatialSpaceViewKind::TwoD,
        );
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
        state.scene_bbox = calculate_bounding_box(parts, &mut state.scene_bbox_accum);
        state.scene_num_primitives = view_ctx
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        crate::ui_2d::view_2d(ctx, ui, state, view_ctx, parts, query, draw_data)
    }
}
