use re_data_store::{EntityPath, InstancePathHash};
use re_query::{ArchetypeView, QueryError};
use re_types::{
    archetypes::Points3D,
    components::{Label, Point3D},
    Archetype as _,
};
use re_viewer_context::{
    ArchetypeDefinition, ResolvedAnnotationInfos, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
};

use itertools::Itertools as _;

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::{
        entity_iterator::process_archetype_views, load_keypoint_connections,
        process_annotations_and_keypoints, process_colors, process_radii, UiLabel, UiLabelTarget,
    },
    view_kind::SpatialSpaceViewKind,
};

use super::{picking_id_from_instance_key, SpatialViewPartData};

pub struct Points3DPart {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewPartData,
}

impl Default for Points3DPart {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewPartData::new(Some(SpatialSpaceViewKind::ThreeD)),
        }
    }
}

impl Points3DPart {
    fn process_labels<'a>(
        arch_view: &'a ArchetypeView<Points3D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        re_tracing::profile_function!();
        let labels = itertools::izip!(
            annotation_infos.iter(),
            arch_view.iter_required_component::<Point3D>()?,
            arch_view.iter_optional_component::<Label>()?,
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
                        target: UiLabelTarget::Position3D(
                            world_from_obj.transform_point3(point.into()),
                        ),
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
        arch_view: &ArchetypeView<Points3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) = process_annotations_and_keypoints::<Point3D, Points3D>(
            query,
            arch_view,
            &ent_context.annotations,
            |p| (*p).into(),
        )?;

        let colors = process_colors(arch_view, ent_path, &annotation_infos)?;
        let radii = process_radii(arch_view, ent_path)?;

        if arch_view.num_instances() <= self.max_labels {
            re_tracing::profile_scope!("labels");

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
                ent_context.world_from_obj,
            )?);
        }

        {
            let mut point_builder = ent_context.shared_render_builders.points();
            let point_batch = point_builder
                .batch("3d points")
                .world_from_obj(ent_context.world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let (positions, radii, colors, picking_instance_ids) = join4(
                || {
                    re_tracing::profile_scope!("positions");
                    arch_view
                        .iter_required_component::<Point3D>()
                        .map(|p| p.map(glam::Vec3::from).collect_vec())
                },
                || {
                    re_tracing::profile_scope!("radii");
                    radii.collect_vec()
                },
                || {
                    re_tracing::profile_scope!("colors");
                    colors.collect_vec()
                },
                || {
                    re_tracing::profile_scope!("picking_ids");
                    arch_view
                        .iter_instance_keys()
                        .map(picking_id_from_instance_key)
                        .collect_vec()
                },
            );

            let positions = positions?;

            let mut point_range_builder =
                point_batch.add_points(&positions, &radii, &colors, &picking_instance_ids);

            // Determine if there's any sub-ranges that need extra highlighting.
            {
                re_tracing::profile_scope!("marking additional highlight points");
                for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                    // TODO(andreas/jeremy): We can do this much more efficiently
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

            self.data.extend_bounding_box_with_points(
                positions.iter().copied(),
                ent_context.world_from_obj,
            );
        }

        load_keypoint_connections(ent_context, ent_path, &keypoints);

        Ok(())
    }
}

impl ViewPartSystem for Points3DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        Points3D::all_components().try_into().unwrap()
    }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        re_tracing::profile_scope!("Points3DPart");

        process_archetype_views::<Points3D, { Points3D::NUM_COMPONENTS }, _>(
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

/// Run 4 things in parallel
fn join4<A: Send, B: Send, C: Send, D: Send>(
    a: impl FnOnce() -> A + Send,
    b: impl FnOnce() -> B + Send,
    c: impl FnOnce() -> C + Send,
    d: impl FnOnce() -> D + Send,
) -> (A, B, C, D) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        re_tracing::profile_function!();
        let ((a, b), (c, d)) = rayon::join(|| rayon::join(a, b), || rayon::join(c, d));
        (a, b, c, d)
    }

    #[cfg(target_arch = "wasm32")]
    {
        (a(), b(), c(), d())
    }
}
