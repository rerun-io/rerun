use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::TimeInt;
use re_query::{ArchetypeView, QueryError};
use re_renderer::PickingLayerInstanceId;
use re_types::{
    archetypes::Points3D,
    components::{ClassId, Color, InstanceKey, KeypointId, Position3D, Radius, Text},
    Archetype as _, ComponentNameSet, DeserializationResult,
};
use re_viewer_context::{
    Annotations, IdentifiedViewSystem, ResolvedAnnotationInfos, SpaceViewSystemExecutionError,
    ViewContextCollection, ViewPartSystem, ViewQuery, ViewerContext,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    parts::{
        load_keypoint_connections, process_annotations_and_keypoints,
        process_cached_annotations_and_keypoints, process_cached_colors, process_cached_radii,
        process_colors, process_radii, UiLabel, UiLabelTarget,
    },
    view_kind::SpatialSpaceViewKind,
};

use super::{
    entity_iterator::process_cached_archetype_views_r1o5, picking_id_from_instance_key, Keypoints,
    SpatialViewPartData,
};

// TODO
pub struct Points3DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub positions: &'a [Position3D],
    pub colors: &'a [Option<Color>],
    pub radii: &'a [Option<Radius>],
    pub labels: &'a [Option<Text>],
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
}

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
        &Points3DComponentData {
            positions, labels, ..
        }: &'a Points3DComponentData<'_>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        re_tracing::profile_function!();
        let labels = itertools::izip!(
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
                        target: UiLabelTarget::Position3D(
                            world_from_obj.transform_point3((*point).into()),
                        ),
                        labeled_instance: *labeled_instance,
                    }),
                    _ => None,
                }
            },
        );
        Ok(labels)
    }

    fn process_cached_data(
        &mut self,
        query: &ViewQuery<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data @ &Points3DComponentData { instance_keys, .. }: &Points3DComponentData<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let LoadedPoints {
            annotation_infos,
            keypoints,
            positions,
            radii,
            colors,
            picking_instance_ids,
        } = LoadedPoints::load_cached(data, ent_path, query.latest_at, &ent_context.annotations)?;

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
                    // TODO
                    let highlighted_point_index =
                        instance_keys.iter().position(|key| highlighted_key == key);
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

        if instance_keys.len() <= self.max_labels {
            re_tracing::profile_scope!("labels");

            // Max labels is small enough that we can afford iterating on the colors again.
            let colors = process_cached_colors(data.colors, ent_path, &annotation_infos)?
                .collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                instance_keys
                    .iter()
                    .copied()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            self.data.ui_labels.extend(Self::process_labels(
                data,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                ent_context.world_from_entity,
            )?);
            // TODO: xxx
        }

        Ok(())
    }
}

impl IdentifiedViewSystem for Points3DPart {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Points3D".into()
    }
}

impl ViewPartSystem for Points3DPart {
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
        process_cached_archetype_views_r1o5::<
            Points3DPart,
            { Points3D::NUM_COMPONENTS },
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
                self.process_cached_data(query, ent_path, ent_context, &data)
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

#[doc(hidden)] // Public for benchmarks
pub struct LoadedPoints {
    pub annotation_infos: ResolvedAnnotationInfos,
    pub keypoints: Keypoints,
    pub positions: Vec<glam::Vec3>,
    pub radii: Vec<re_renderer::Size>,
    pub colors: Vec<re_renderer::Color32>,
    pub picking_instance_ids: Vec<PickingLayerInstanceId>,
}

impl LoadedPoints {
    #[inline]
    pub fn load_cached(
        data @ &Points3DComponentData {
            positions,
            keypoint_ids,
            class_ids,
            ..
        }: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
        latest_at: TimeInt,
        annotations: &Annotations,
    ) -> Result<Self, QueryError> {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) = process_cached_annotations_and_keypoints::<Position3D>(
            latest_at,
            positions,
            keypoint_ids,
            class_ids,
            annotations,
            |p| (*p).into(),
        )?;

        // TODO: rayon overhead is not worth it at this point
        let positions = Self::load_cached_positions(data);
        let radii = Self::load_cached_radii(data, ent_path);
        let colors = Self::load_cached_colors(data, ent_path, &annotation_infos);
        let picking_instance_ids = Self::load_cached_picking_ids(data);

        Ok(Self {
            annotation_infos,
            keypoints,
            positions,
            radii: radii?,
            colors: colors?,
            picking_instance_ids,
        })
    }

    #[inline]
    pub fn load_cached_positions(
        &Points3DComponentData { positions, .. }: &Points3DComponentData<'_>,
    ) -> Vec<glam::Vec3> {
        re_tracing::profile_function!();
        // TODO: why is this not a bytemuck cast tho?
        positions.iter().copied().map(glam::Vec3::from).collect()
    }

    #[inline]
    pub fn load_cached_radii(
        &Points3DComponentData { radii, .. }: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
    ) -> Result<Vec<re_renderer::Size>, QueryError> {
        re_tracing::profile_function!();
        // TODO: why is this not a bytemuck cast tho?
        process_cached_radii(radii, ent_path).map(|radii| {
            re_tracing::profile_scope!("collect");
            radii.collect()
        })
    }

    #[inline]
    pub fn load_cached_colors(
        &Points3DComponentData { colors, .. }: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
        annotation_infos: &ResolvedAnnotationInfos,
    ) -> Result<Vec<re_renderer::Color32>, QueryError> {
        re_tracing::profile_function!();
        process_cached_colors(colors, ent_path, annotation_infos).map(|colors| {
            re_tracing::profile_scope!("collect");
            colors.collect()
        })
    }

    #[inline]
    pub fn load_cached_picking_ids(
        &Points3DComponentData { instance_keys, .. }: &Points3DComponentData<'_>,
    ) -> Vec<PickingLayerInstanceId> {
        re_tracing::profile_function!();

        // TODO: why is this not a bytemuck cast tho?
        let iterator = instance_keys
            .iter()
            .copied()
            .map(picking_id_from_instance_key);

        re_tracing::profile_scope!("collect");
        iterator.collect()
    }
}

impl LoadedPoints {
    #[inline]
    pub fn load(
        arch_view: &ArchetypeView<Points3D>,
        ent_path: &EntityPath,
        latest_at: TimeInt,
        annotations: &Annotations,
    ) -> Result<Self, QueryError> {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) = process_annotations_and_keypoints::<
            Position3D,
            Points3D,
        >(latest_at, arch_view, annotations, |p| {
            (*p).into()
        })?;

        let (positions, radii, colors, picking_instance_ids) = join4(
            || Self::load_positions(arch_view),
            || Self::load_radii(arch_view, ent_path),
            || Self::load_colors(arch_view, ent_path, &annotation_infos),
            || Self::load_picking_ids(arch_view),
        );

        Ok(Self {
            annotation_infos,
            keypoints,
            positions: positions?,
            radii: radii?,
            colors: colors?,
            picking_instance_ids,
        })
    }

    #[inline]
    pub fn load_positions(
        arch_view: &ArchetypeView<Points3D>,
    ) -> DeserializationResult<Vec<glam::Vec3>> {
        re_tracing::profile_function!();
        arch_view.iter_required_component::<Position3D>().map(|p| {
            re_tracing::profile_scope!("collect");
            p.map(glam::Vec3::from).collect()
        })
    }

    #[inline]
    pub fn load_radii(
        arch_view: &ArchetypeView<Points3D>,
        ent_path: &EntityPath,
    ) -> Result<Vec<re_renderer::Size>, QueryError> {
        re_tracing::profile_function!();
        process_radii(arch_view, ent_path).map(|radii| {
            re_tracing::profile_scope!("collect");
            radii.collect()
        })
    }

    #[inline]
    pub fn load_colors(
        arch_view: &ArchetypeView<Points3D>,
        ent_path: &EntityPath,
        annotation_infos: &ResolvedAnnotationInfos,
    ) -> Result<Vec<re_renderer::Color32>, QueryError> {
        re_tracing::profile_function!();
        process_colors(arch_view, ent_path, annotation_infos).map(|colors| {
            re_tracing::profile_scope!("collect");
            colors.collect()
        })
    }

    #[inline]
    pub fn load_picking_ids(arch_view: &ArchetypeView<Points3D>) -> Vec<PickingLayerInstanceId> {
        re_tracing::profile_function!();
        let iterator = arch_view
            .iter_instance_keys()
            .map(picking_id_from_instance_key);

        re_tracing::profile_scope!("collect");
        iterator.collect()
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
