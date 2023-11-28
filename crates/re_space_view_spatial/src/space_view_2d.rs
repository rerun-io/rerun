use re_data_store::EntityProperties;
use re_log_types::EntityPath;
use re_viewer_context::{
    AutoSpawnHeuristic, PerSystemEntities, SpaceViewClass, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartCollection,
    ViewQuery, ViewerContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{auto_spawn_heuristic, update_object_property_heuristics},
    parts::{calculate_bounding_box, register_2d_spatial_parts, SpatialViewPartData},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
};

#[derive(Default)]
pub struct SpatialSpaceView2D;

impl SpatialSpaceView2D {
    pub const NAME: &'static str = "2D";
}

impl SpaceViewClass for SpatialSpaceView2D {
    type State = SpatialSpaceViewState;

    fn name(&self) -> re_viewer_context::SpaceViewClassName {
        Self::NAME.into()
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
        space_origin: &EntityPath,
        per_system_entities: &PerSystemEntities,
    ) -> AutoSpawnHeuristic {
        let mut score = auto_spawn_heuristic(
            &self.name(),
            ctx,
            per_system_entities,
            SpatialSpaceViewKind::TwoD,
        );

        // If this is the root space view, and it contains a part that would
        // prefer to be 3D, don't spawn the 2D view.
        //
        // Since pinhole projections provide a mapping between a 2D child space and a 3D
        // parent space, it means that for any 3D content to be projected into a 2D space,
        // there must exist a common 3D ancestor-space which has a *child* which is a 2D space
        // (and contains a pinhole.) But if this space origin is at the root itself, that common
        // ancestor would need to be a parent of the root, which doesn't exist. As such, it's
        // impossible that this space would be able to correctly account for the required
        // content. By not spawning a 2D space in this case we ensure the 3D version gets chosen
        // by the heuristics instead.
        if space_origin.is_root() {
            let parts = ctx
                .space_view_class_registry
                .get_system_registry_or_log_error(&self.name())
                .new_part_collection();

            for part in per_system_entities.keys() {
                if let Ok(part) = parts.get_by_name(*part) {
                    if let Some(part_data) = part
                        .data()
                        .and_then(|d| d.downcast_ref::<SpatialViewPartData>())
                    {
                        if part_data.preferred_view_kind == Some(SpatialSpaceViewKind::ThreeD) {
                            return AutoSpawnHeuristic::NeverSpawn;
                        }
                    }
                }
            }
        }

        if let AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score) = &mut score {
            // If a 2D view contains nothing inherently 2D in nature, don't
            // spawn it, though in all other cases default to 2D over 3D as a tie-breaker.
            if *score == 0.0 {
                return AutoSpawnHeuristic::NeverSpawn;
            } else {
                *score += 0.1;
            }
        }

        score
    }

    fn selection_ui(
        &self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
        state.selection_ui(ctx, ui, space_origin, SpatialSpaceViewKind::TwoD);
    }

    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
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
