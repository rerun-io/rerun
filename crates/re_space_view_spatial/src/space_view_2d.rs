use nohash_hasher::IntSet;
use re_log_types::EntityPath;
use re_viewer_context::{
    AutoSpawnHeuristic, SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewPartCollection, ViewQuery,
    ViewerContext,
};

use crate::{
    contexts::{register_contexts, PrimitiveCounter},
    heuristics::auto_spawn_heuristic,
    parts::{calculate_bounding_box, register_parts},
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
        register_contexts(system_registry)?;
        register_parts(system_registry)?;
        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, state: &Self::State) -> Option<f32> {
        let size = state.scene_bbox_accum.size();
        Some(size.x / size.y)
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
    }

    fn prepare_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        state: &Self::State,
        ent_paths: &IntSet<EntityPath>,
        entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
        state.update_object_property_heuristics(
            ctx,
            ent_paths,
            entity_properties,
            SpatialSpaceViewKind::TwoD,
        );
    }

    fn auto_spawn_heuristic(
        &self,
        ctx: &ViewerContext<'_>,
        _space_origin: &EntityPath,
        ent_paths: &IntSet<EntityPath>,
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
        // TODO: Duplicated with 3d
        state.scene_bbox = calculate_bounding_box(parts);
        if state.scene_bbox_accum.is_nothing() || !state.scene_bbox_accum.size().is_finite() {
            state.scene_bbox_accum = state.scene_bbox;
        } else {
            state.scene_bbox_accum = state.scene_bbox_accum.union(state.scene_bbox);
        }

        state.scene_num_primitives = view_ctx
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        let scene_rect_accum = egui::Rect::from_min_max(
            state.scene_bbox_accum.min.truncate().to_array().into(),
            state.scene_bbox_accum.max.truncate().to_array().into(),
        );
        crate::ui_2d::view_2d(
            ctx,
            ui,
            state,
            view_ctx,
            parts,
            query,
            scene_rect_accum,
            draw_data,
        )
    }
}
