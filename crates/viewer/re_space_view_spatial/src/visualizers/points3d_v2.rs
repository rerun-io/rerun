use itertools::Itertools;

use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PointCloudBuilder};
use re_space_view::{process_annotation_and_keypoint_slices, process_color_slice};
use re_types::{
    archetypes::Points3D,
    components::{ClassId, Color, KeypointId, Position3D, Radius, ShowLabels, Text},
    ArrowString, Component,
};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialSpaceViewKind,
    visualizers::{load_keypoint_connections, process_radius_slice},
};

use super::{
    filter_visualizable_3d_entities, process_labels_3d, utilities::LabeledBatch,
    SpatialViewVisualizerData,
};

// ---

pub struct Points3DVisualizer2 {}

impl Default for Points3DVisualizer2 {
    fn default() -> Self {
        Self {}
    }
}

impl IdentifiedViewSystem for Points3DVisualizer2 {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Points3D_v2".into()
    }
}

impl VisualizerSystem for Points3DVisualizer2 {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Points3D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        #[cfg(TODO)]
        for data_result in query.iter_visible_data_results(ctx, system_identifier) {
            let Some(transform_info) =
                transforms.transform_info_for_entity(&data_result.entity_path)
            else {
                continue;
            };

            let depth_offset_key = (system_identifier, data_result.entity_path.hash());
            let entity_context = SpatialSceneEntityContext {
                transform_info,
                depth_offset: depth_offsets
                    .per_entity_and_visualizer
                    .get(&depth_offset_key)
                    .copied()
                    .unwrap_or_default(),
                annotations: annotations.0.find(&data_result.entity_path),
                highlight: query
                    .highlights
                    .entity_outline_mask(data_result.entity_path.hash()),
                space_view_class_identifier: view_ctx.space_view_class_identifier(),
            };

            let results = data_result.query_archetype_with_history::<A>(ctx, query);

            let mut query_ctx = ctx.query_context(data_result, &latest_at);
            query_ctx.archetype_name = Some(A::name());

            {
                re_tracing::profile_scope!(format!("{}", data_result.entity_path));
                fun(&query_ctx, &entity_context, &results)?;
            }
        }

        Ok(Vec::new())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(Points3DVisualizer2 => []);
