use ahash::HashMap;

use re_entity_db::{EntityPath, InstancePathHash};
use re_log_types::TimeInt;
use re_renderer::PickingLayerInstanceId;
use re_types::{
    archetypes::Points3D,
    components::{ClassId, Color, InstanceKey, KeypointId, Position3D, Radius, Text},
    Archetype as _, ComponentNameSet,
};
use re_viewer_context::{
    Annotations, IdentifiedViewSystem, ResolvedAnnotationInfos, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewQuery, ViewerContext, VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{load_keypoint_connections, process_color_slice, UiLabel, UiLabelTarget},
};

use super::{Keypoints, SpatialViewVisualizerData};

// ---

pub struct Points3DVisualizer {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Points3DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::ThreeD)),
        }
    }
}

impl Points3DVisualizer {
    fn process_labels<'a>(
        &Points3DComponentData { labels, .. }: &'a Points3DComponentData<'_>,
        positions: &'a [glam::Vec3],
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        re_tracing::profile_function!();
        itertools::izip!(
            annotation_infos.iter(),
            positions,
            labels,
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
                        target: UiLabelTarget::Position3D(world_from_obj.transform_point3(*point)),
                        labeled_instance: *labeled_instance,
                    }),
                    _ => None,
                }
            },
        )
    }

    fn process_data(
        &mut self,
        query: &ViewQuery<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: &Points3DComponentData<'_>,
    ) {
        re_tracing::profile_function!();

        let LoadedPoints {
            annotation_infos,
            keypoints,
            positions,
            radii,
            colors,
            picking_instance_ids,
        } = LoadedPoints::load(data, ent_path, query.latest_at, &ent_context.annotations);

        {
            re_tracing::profile_scope!("to_gpu");

            let mut point_builder = ent_context.shared_render_builders.points();
            let point_batch = point_builder
                .batch("3d points")
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let mut point_range_builder =
                point_batch.add_points(&positions, &radii, &colors, &picking_instance_ids);

            // Determine if there's any sub-ranges that need extra highlighting.
            {
                re_tracing::profile_scope!("marking additional highlight points");
                for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                    // TODO(andreas, jeremy): We can do this much more efficiently
                    let highlighted_point_index = data
                        .instance_keys
                        .iter()
                        .position(|key| highlighted_key == key);
                    if let Some(highlighted_point_index) = highlighted_point_index {
                        point_range_builder = point_range_builder
                            .push_additional_outline_mask_ids_for_range(
                                highlighted_point_index as u32..highlighted_point_index as u32 + 1,
                                *instance_mask_ids,
                            );
                    }
                }
            }
        }

        self.data.extend_bounding_box_with_points(
            positions.iter().copied(),
            ent_context.world_from_entity,
        );

        load_keypoint_connections(ent_context, ent_path, &keypoints);

        if data.instance_keys.len() <= self.max_labels {
            re_tracing::profile_scope!("labels");

            // Max labels is small enough that we can afford iterating on the colors again.
            let colors =
                process_color_slice(data.colors, ent_path, &annotation_infos).collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                data.instance_keys
                    .iter()
                    .copied()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            self.data.ui_labels.extend(Self::process_labels(
                data,
                &positions,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                ent_context.world_from_entity,
            ));
        }
    }
}

impl IdentifiedViewSystem for Points3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Points3D".into()
    }
}

impl VisualizerSystem for Points3DVisualizer {
    fn required_components(&self) -> ComponentNameSet {
        Points3D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Points3D::indicator().name()).collect()
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        super::entity_iterator::process_archetype_pov1_comp5::<
            Points3DVisualizer,
            Points3D,
            Position3D,
            Color,
            Radius,
            Text,
            re_types::components::KeypointId,
            re_types::components::ClassId,
            _,
        >(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            ctx.app_options.experimental_primary_caching_point_clouds,
            |_ctx,
             ent_path,
             _ent_props,
             ent_context,
             (_time, _row_id),
             instance_keys,
             positions,
             colors,
             radii,
             labels,
             keypoint_ids,
             class_ids| {
                let data = Points3DComponentData {
                    instance_keys,
                    positions,
                    colors,
                    radii,
                    labels,
                    keypoint_ids: keypoint_ids
                        .iter()
                        .any(Option::is_some)
                        .then_some(keypoint_ids),
                    class_ids: class_ids.iter().any(Option::is_some).then_some(class_ids),
                };
                self.process_data(query, ent_path, ent_context, &data);
                Ok(())
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

// ---

#[doc(hidden)] // Public for benchmarks
pub struct LoadedPoints {
    pub annotation_infos: ResolvedAnnotationInfos,
    pub keypoints: Keypoints,
    pub positions: Vec<glam::Vec3>,
    pub radii: Vec<re_renderer::Size>,
    pub colors: Vec<re_renderer::Color32>,
    pub picking_instance_ids: Vec<PickingLayerInstanceId>,
}

#[doc(hidden)] // Public for benchmarks
pub struct Points3DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub positions: &'a [Position3D],
    pub colors: &'a [Option<Color>],
    pub radii: &'a [Option<Radius>],
    pub labels: &'a [Option<Text>],
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
}

impl LoadedPoints {
    #[inline]
    pub fn load(
        data: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
        latest_at: TimeInt,
        annotations: &Annotations,
    ) -> Self {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) =
            Self::process_annotations_and_keypoints(latest_at, data, annotations);

        let (positions, radii, colors, picking_instance_ids) = join4(
            || Self::load_positions(data),
            || Self::load_radii(data, ent_path),
            || Self::load_colors(data, ent_path, &annotation_infos),
            || Self::load_picking_ids(data),
        );

        Self {
            annotation_infos,
            keypoints,
            positions,
            radii,
            colors,
            picking_instance_ids,
        }
    }

    #[inline]
    pub fn load_positions(
        Points3DComponentData { positions, .. }: &Points3DComponentData<'_>,
    ) -> Vec<glam::Vec3> {
        re_tracing::profile_function!();
        bytemuck::cast_slice(positions).to_vec()
    }

    #[inline]
    pub fn load_radii(
        &Points3DComponentData { radii, .. }: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
    ) -> Vec<re_renderer::Size> {
        re_tracing::profile_function!();
        let radii = crate::visualizers::process_radius_slice(radii, ent_path);
        {
            re_tracing::profile_scope!("collect");
            radii.collect()
        }
    }

    #[inline]
    pub fn load_colors(
        &Points3DComponentData { colors, .. }: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
        annotation_infos: &ResolvedAnnotationInfos,
    ) -> Vec<re_renderer::Color32> {
        re_tracing::profile_function!();
        let colors = crate::visualizers::process_color_slice(colors, ent_path, annotation_infos);
        {
            re_tracing::profile_scope!("collect");
            colors.collect()
        }
    }

    #[inline]
    pub fn load_picking_ids(
        &Points3DComponentData { instance_keys, .. }: &Points3DComponentData<'_>,
    ) -> Vec<PickingLayerInstanceId> {
        re_tracing::profile_function!();
        bytemuck::cast_slice(instance_keys).to_vec()
    }

    /// Resolves all annotations and keypoints for the given entity view.
    fn process_annotations_and_keypoints(
        latest_at: re_log_types::TimeInt,
        data @ &Points3DComponentData {
            instance_keys,
            keypoint_ids,
            class_ids,
            ..
        }: &Points3DComponentData<'_>,
        annotations: &Annotations,
    ) -> (ResolvedAnnotationInfos, Keypoints) {
        re_tracing::profile_function!();

        let mut keypoints: Keypoints = HashMap::default();

        // No need to process annotations if we don't have keypoints or class-ids
        let (Some(keypoint_ids), Some(class_ids)) = (keypoint_ids, class_ids) else {
            let resolved_annotation = annotations
                .resolved_class_description(None)
                .annotation_info();

            return (
                ResolvedAnnotationInfos::Same(instance_keys.len(), resolved_annotation),
                keypoints,
            );
        };

        let annotation_info = itertools::izip!(data.positions.iter(), keypoint_ids, class_ids)
            .map(|(positions, &keypoint_id, &class_id)| {
                let class_description = annotations.resolved_class_description(class_id);

                if let (Some(keypoint_id), Some(class_id), position) =
                    (keypoint_id, class_id, positions)
                {
                    keypoints
                        .entry((class_id, latest_at.as_i64()))
                        .or_default()
                        .insert(keypoint_id.0, position.0.into());
                    class_description.annotation_info_with_keypoint(keypoint_id.0)
                } else {
                    class_description.annotation_info()
                }
            })
            .collect();

        (ResolvedAnnotationInfos::Many(annotation_info), keypoints)
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
