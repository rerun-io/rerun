use re_data_store::EntityProperties;
use re_log_types::EntityPath;
use re_viewer_context::{
    AutoSpawnHeuristic, IdentifiedViewSystem as _, PerSystemEntities, SpaceViewClass,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartCollection, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{auto_spawn_heuristic, update_object_property_heuristics},
    parts::{calculate_bounding_box, register_3d_spatial_parts, CamerasPart},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
};

#[derive(Default)]
pub struct SpatialSpaceView3D;

impl SpaceViewClass for SpatialSpaceView3D {
    type State = SpatialSpaceViewState;

    const IDENTIFIER: &'static str = "3D";
    const DISPLAY_NAME: &'static str = "3D";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_3D
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        super::ui_3d::help_text(re_ui)
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        register_spatial_contexts(system_registry)?;
        register_3d_spatial_parts(system_registry)?;

        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
    }

    fn auto_spawn_heuristic(
        &self,
        ctx: &ViewerContext<'_>,
        space_origin: &EntityPath,
        per_system_entities: &PerSystemEntities,
    ) -> AutoSpawnHeuristic {
        let score = auto_spawn_heuristic(
            &self.identifier(),
            ctx,
            per_system_entities,
            SpatialSpaceViewKind::ThreeD,
        );

        if let AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(mut score) = score {
            if let Some(camera_paths) = per_system_entities.get(&CamerasPart::identifier()) {
                // If there is a camera at the origin, this cannot be a 3D space -- it must be 2D
                if camera_paths.contains(space_origin) {
                    return AutoSpawnHeuristic::NeverSpawn;
                } else if !camera_paths.is_empty() {
                    // If there's a camera at a non-root path, make 3D view higher priority.
                    // TODO(andreas): It would be nice to just return `AutoSpawnHeuristic::AlwaysSpawn` here
                    // but AlwaysSpawn does not prevent other `SpawnClassWithHighestScoreForRoot` instances
                    // from being added to the view.
                    score += 100.0;
                }
            }

            AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(score)
        } else {
            score
        }
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
            SpatialSpaceViewKind::ThreeD,
        );
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
        state.selection_ui(ctx, ui, space_origin, SpatialSpaceViewKind::ThreeD);
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
        re_tracing::profile_function!();

        state.scene_bbox = calculate_bounding_box(parts, &mut state.scene_bbox_accum);
        state.scene_num_primitives = view_ctx
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        crate::ui_3d::view_3d(ctx, ui, state, view_ctx, parts, query, draw_data)
    }
}
