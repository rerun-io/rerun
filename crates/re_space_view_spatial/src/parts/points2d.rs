use re_data_store::{EntityPath, InstancePathHash};
use re_query::{ArchetypeView, QueryError};
use re_types::{
    archetypes::Points2D,
    components::{Position2D, Text},
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    NamedViewSystem, ResolvedAnnotationInfos, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewPartSystem, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::{
        entity_iterator::process_archetype_views, load_keypoint_connections,
        process_annotations_and_keypoints, process_colors, process_radii, UiLabel, UiLabelTarget,
    },
    view_kind::SpatialSpaceViewKind,
};

use super::{picking_id_from_instance_key, SpatialViewPartData};

pub struct Points2DPart {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewPartData,
}

impl Default for Points2DPart {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewPartData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl Points2DPart {
    fn process_labels<'a>(
        arch_view: &'a ArchetypeView<Points2D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        let labels = itertools::izip!(
            annotation_infos.iter(),
            arch_view.iter_required_component::<Position2D>()?,
            arch_view.iter_optional_component::<Text>()?,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, point, label, color, labeled_instance)| {
                let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));
                match (point, label) {
                    (point, Some(label)) => Some(UiLabel {
                        text: label,
                        color: *color,
                        target: UiLabelTarget::Point2D(egui::pos2(point.x(), point.y())),
                        labeled_instance: *labeled_instance,
                    }),
                    _ => None,
                }
            },
        );
        Ok(labels)
    }

    fn process_arch_view(
        &mut self,
        query: &ViewQuery<'_>,
        arch_view: &ArchetypeView<Points2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) =
            process_annotations_and_keypoints::<Position2D, Points2D>(
                query,
                arch_view,
                &ent_context.annotations,
                |p| (*p).into(),
            )?;

        let colors = process_colors(arch_view, ent_path, &annotation_infos)?;
        let radii = process_radii(arch_view, ent_path)?;

        if arch_view.num_instances() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors =
                process_colors(arch_view, ent_path, &annotation_infos)?.collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                arch_view
                    .iter_instance_keys()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            self.data.ui_labels.extend(Self::process_labels(
                arch_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
            )?);
        }

        {
            let mut point_builder = ent_context.shared_render_builders.points();
            let point_batch = point_builder
                .batch("2d points")
                .depth_offset(ent_context.depth_offset)
                .flags(
                    re_renderer::renderer::PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES
                        | re_renderer::renderer::PointCloudBatchFlags::FLAG_ENABLE_SHADING,
                )
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let positions = arch_view
                .iter_required_component::<Position2D>()?
                .map(|pt| pt.into());

            let picking_instance_ids = arch_view
                .iter_instance_keys()
                .map(picking_id_from_instance_key);

            let positions: Vec<glam::Vec3> = {
                re_tracing::profile_scope!("collect_positions");
                positions.collect()
            };
            let radii: Vec<_> = {
                re_tracing::profile_scope!("collect_radii");
                radii.collect()
            };
            let colors: Vec<_> = {
                re_tracing::profile_scope!("collect_colors");
                colors.collect()
            };
            let picking_instance_ids: Vec<_> = {
                re_tracing::profile_scope!("collect_picking_instance_ids");
                picking_instance_ids.collect()
            };

            let mut point_range_builder =
                point_batch.add_points_2d(&positions, &radii, &colors, &picking_instance_ids);

            // Determine if there's any sub-ranges that need extra highlighting.
            {
                re_tracing::profile_scope!("marking additional highlight points");
                for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                    // TODO(andreas, jeremy): We can do this much more efficiently
                    let highlighted_point_index = arch_view
                        .iter_instance_keys()
                        .position(|key| *highlighted_key == key);
                    if let Some(highlighted_point_index) = highlighted_point_index {
                        point_range_builder = point_range_builder
                            .push_additional_outline_mask_ids_for_range(
                                highlighted_point_index as u32..highlighted_point_index as u32 + 1,
                                *instance_mask_ids,
                            );
                    }
                }
            }
        };

        load_keypoint_connections(ent_context, ent_path, &keypoints);

        self.data.extend_bounding_box_with_points(
            arch_view
                .iter_required_component::<Position2D>()?
                .map(|pt| pt.into()),
            ent_context.world_from_entity,
        );

        Ok(())
    }
}

impl NamedViewSystem for Points2DPart {
    fn name() -> re_viewer_context::ViewSystemName {
        "Points2D".into()
    }
}

impl ViewPartSystem for Points2DPart {
    fn required_components(&self) -> ComponentNameSet {
        Points2D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Points2D::indicator_component()).collect()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        process_archetype_views::<Points2DPart, Points2D, { Points2D::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |_ctx, ent_path, arch_view, ent_context| {
                self.process_arch_view(query, &arch_view, ent_path, ent_context)
            },
        )?;

        Ok(Vec::new()) // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
