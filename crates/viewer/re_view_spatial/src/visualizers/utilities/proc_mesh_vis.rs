use re_entity_db::InstancePathHash;
use re_log_types::Instance;
use re_renderer::renderer::{GpuMeshInstance, LineStripFlags};
use re_renderer::{PickingLayerInstanceId, RenderContext};
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::components::{self, FillMode};
use re_tf::convert;
use re_view::{clamped_or_nothing, process_annotation_slices, process_color_slice};
#[cfg(doc)]
use re_viewer_context::VisualizerSystem;
use re_viewer_context::{QueryContext, ViewQuery, ViewSystemExecutionError, typed_fallback_for};
use vec1::smallvec_v1::SmallVec1;

use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::proc_mesh::{self, ProcMeshKey};
use crate::visualizers::utilities::LabeledBatch;
use crate::visualizers::{SpatialViewVisualizerData, process_labels_3d, process_radius_slice};

/// To be used within the scope of a single [`VisualizerSystem::execute()`] call
/// when the visualizer wishes to draw batches of [`ProcMeshKey`] meshes.
pub struct ProcMeshDrawableBuilder<'ctx> {
    /// Bounding box and label info here will be updated by the drawn batches.
    pub data: &'ctx mut SpatialViewVisualizerData,

    /// Accumulates lines to render.
    /// TODO(kpreid): Should be using instanced meshes kept in GPU buffers
    /// instead of this immediate-mode strategy that copies every vertex every frame.
    pub line_builder: re_renderer::LineDrawableBuilder<'ctx>,
    pub line_batch_debug_label: re_renderer::DebugLabel,

    /// Accumulates triangle mesh instances to render.
    pub solid_instances: Vec<GpuMeshInstance>,

    pub query: &'ctx ViewQuery<'ctx>,
    pub render_ctx: &'ctx RenderContext,
}

/// A [batch] of instances to draw. This struct is just arguments to
/// [`ProcMeshDrawableBuilder::add_batch()`].
///
/// TODO(#7026): Document how the number of instances is derived from this data.
///
/// [batch]: https://rerun.io/docs/concepts/batches
pub struct ProcMeshBatch<'a, IMesh, IFill> {
    pub half_sizes: &'a [components::HalfSize3D],

    pub centers: &'a [components::Translation3D],
    pub rotation_axis_angles: &'a [components::RotationAxisAngle],
    pub quaternions: &'a [components::RotationQuat],

    /// Iterator of meshes. Must be at least as long as `half_sizes`.
    pub meshes: IMesh,

    /// Iterator of mesh fill modes. Must be at least as long as `half_sizes`.
    pub fill_modes: IFill,

    pub line_radii: &'a [components::Radius],
    pub colors: &'a [components::Color],
    pub labels: &'a [re_sdk_types::ArrowString],
    pub show_labels: Option<components::ShowLabels>,
    pub class_ids: &'a [components::ClassId],
}

/// Combines transform-like components on the entity with instances pose to view-origin transforms
/// provided by the transform system.
// TODO(#7026): We should formalize this kind of hybrid joining better.
fn combine_instance_poses_with_archetype_transforms(
    num_half_sizes: usize,
    target_from_poses: &SmallVec1<[glam::DAffine3; 1]>,
    translations: &[components::Translation3D],
    rotation_axis_angles: &[components::RotationAxisAngle],
    quaternions: &[components::RotationQuat],
) -> vec1::Vec1<glam::DAffine3> {
    // Draw as many proc meshes as we have max(instance pose count, proc mesh count), all components get repeated over that number.
    let num_instances = num_half_sizes
        .max(target_from_poses.len())
        .max(translations.len())
        .max(rotation_axis_angles.len())
        .max(quaternions.len());

    let mut iter_translation = clamped_or_nothing(translations, num_instances);
    let mut iter_rotation_axis_angle = clamped_or_nothing(rotation_axis_angles, num_instances);
    let mut iter_rotation_quat = clamped_or_nothing(quaternions, num_instances);

    let last_target_from_instances = target_from_poses.last();
    let clamped_target_from_instances = target_from_poses
        .iter()
        .chain(std::iter::repeat(last_target_from_instances))
        .copied();

    let target_from_instances = clamped_target_from_instances
        .take(num_instances)
        .map(|mut transform| {
            if let Some(translation) = iter_translation.next() {
                transform *= convert::translation_3d_to_daffine3(*translation);
            }

            if let Some(rotation_axis_angle) = iter_rotation_axis_angle.next() {
                if let Ok(axis_angle) =
                    convert::rotation_axis_angle_to_daffine3(*rotation_axis_angle)
                {
                    transform *= axis_angle;
                } else {
                    transform = glam::DAffine3::ZERO;
                }
            }

            if let Some(rotation_quat) = iter_rotation_quat.next() {
                if let Ok(rotation_quat) = convert::rotation_quat_to_daffine3(*rotation_quat) {
                    transform *= rotation_quat;
                } else {
                    transform = glam::DAffine3::ZERO;
                }
            }

            transform
        })
        .collect();

    vec1::Vec1::try_from_vec(target_from_instances).expect("built from a SmallVec1, so can't fail")
}

impl<'ctx> ProcMeshDrawableBuilder<'ctx> {
    pub fn new(
        data: &'ctx mut SpatialViewVisualizerData,
        render_ctx: &'ctx re_renderer::RenderContext,
        view_query: &'ctx ViewQuery<'ctx>,
        line_batch_debug_label: impl Into<re_renderer::DebugLabel>,
    ) -> Self {
        let mut line_builder = re_renderer::LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        ProcMeshDrawableBuilder {
            data,
            line_builder,
            line_batch_debug_label: line_batch_debug_label.into(),
            solid_instances: Vec::new(),
            query: view_query,
            render_ctx,
        }
    }

    /// Add a batch of data to be drawn.
    pub fn add_batch(
        &mut self,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        color_component: ComponentIdentifier,
        show_labels_component: ComponentIdentifier,
        constant_instance_transform: glam::Affine3A,
        batch: ProcMeshBatch<'_, impl Iterator<Item = ProcMeshKey>, impl Iterator<Item = FillMode>>,
    ) -> Result<(), ViewSystemExecutionError> {
        let entity_path = query_context.target_entity_path;

        if batch.half_sizes.is_empty() {
            return Ok(());
        }

        let target_from_poses = ent_context.transform_info.target_from_instances();
        let target_from_instances = combine_instance_poses_with_archetype_transforms(
            batch.half_sizes.len(),
            target_from_poses,
            batch.centers,
            batch.rotation_axis_angles,
            batch.quaternions,
        );
        let num_instances = target_from_instances.len();

        re_tracing::profile_function_if!(10_000 < num_instances);

        let half_sizes = clamped_or_nothing(batch.half_sizes, num_instances);

        let annotation_infos = process_annotation_slices(
            self.query.latest_at,
            num_instances,
            batch.class_ids,
            &ent_context.annotations,
        );

        // Has not custom fallback for radius, so we use the default.
        // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
        let line_radii = process_radius_slice(
            entity_path,
            num_instances,
            batch.line_radii,
            components::Radius::default(),
        );
        let colors = process_color_slice(
            query_context,
            color_component,
            num_instances,
            &annotation_infos,
            batch.colors,
        );

        let mut line_batch = self
            .line_builder
            .batch(self.line_batch_debug_label.clone())
            .depth_offset(ent_context.depth_offset)
            .outline_mask_ids(ent_context.highlight.overall)
            .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

        let mut world_space_bounding_box = macaw::BoundingBox::nothing();

        let world_from_instances = target_from_instances
            .iter()
            .map(|transform| transform.as_affine3a())
            .chain(std::iter::repeat(
                target_from_instances.last().as_affine3a(),
            ));

        let mut num_instances = 0;
        for (
            instance_index,
            (half_size, world_from_instance, radius, &color, proc_mesh_key, fill_mode),
        ) in itertools::izip!(
            half_sizes,
            world_from_instances,
            line_radii,
            colors.iter(),
            batch.meshes,
            batch.fill_modes
        )
        .enumerate()
        {
            let instance = Instance::from(instance_index as u64);
            num_instances = instance_index + 1;

            let world_from_instance = world_from_instance
                * glam::Affine3A::from_scale(glam::Vec3::from(*half_size))
                * constant_instance_transform;
            world_space_bounding_box = world_space_bounding_box.union(
                proc_mesh_key
                    .simple_bounding_box()
                    .transform_affine3(&world_from_instance),
            );

            match fill_mode {
                FillMode::MajorWireframe | FillMode::DenseWireframe => {
                    let Some(wireframe_mesh) = query_context.store_ctx().caches.entry(
                        |c: &mut proc_mesh::WireframeCache| c.entry(proc_mesh_key, self.render_ctx),
                    ) else {
                        return Err(ViewSystemExecutionError::DrawDataCreationError(
                            "Failed to allocate wireframe mesh".into(),
                        ));
                    };

                    for strip in &wireframe_mesh.line_strips {
                        let strip_builder = line_batch
                            .add_strip(
                                strip
                                    .iter()
                                    .map(|&point| world_from_instance.transform_point3(point)),
                            )
                            .color(color)
                            .radius(radius)
                            .picking_instance_id(PickingLayerInstanceId(instance_index as _))
                            // Looped lines should be connected with rounded corners.
                            .flags(LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS);

                        if let Some(outline_mask_ids) = ent_context
                            .highlight
                            .instances
                            .get(&Instance::from(instance_index as u64))
                        {
                            // Not using ent_context.highlight.index_outline_mask() because
                            // that's already handled when the builder was created.
                            strip_builder.outline_mask_ids(*outline_mask_ids);
                        }
                    }
                }
                FillMode::Solid => {
                    let store_ctx = query_context.store_ctx();
                    let Some(solid_mesh) =
                        store_ctx.caches.entry(|c: &mut proc_mesh::SolidCache| {
                            c.entry(proc_mesh_key, self.render_ctx)
                        })
                    else {
                        return Err(ViewSystemExecutionError::DrawDataCreationError(
                            "Failed to allocate solid mesh".into(),
                        ));
                    };

                    self.solid_instances.push(GpuMeshInstance {
                        gpu_mesh: solid_mesh.gpu_mesh,
                        world_from_mesh: world_from_instance,
                        outline_mask_ids: ent_context.highlight.index_outline_mask(instance),
                        picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                            InstancePathHash::instance(entity_path, instance),
                        ),
                        additive_tint: color,
                    });
                }
            }
        }

        self.data
            .bounding_boxes
            .push((entity_path.hash(), world_space_bounding_box));

        self.data.ui_labels.extend(process_labels_3d(
            LabeledBatch {
                entity_path,
                num_instances,
                overall_position: world_space_bounding_box.center(),
                instance_positions: target_from_instances
                    .iter()
                    .chain(std::iter::repeat(target_from_instances.last()))
                    .map(|t| t.translation.as_vec3()),
                labels: batch.labels,
                colors: &colors,
                show_labels: batch
                    .show_labels
                    .unwrap_or_else(|| typed_fallback_for(query_context, show_labels_component)),
                annotation_infos: &annotation_infos,
            },
            glam::Affine3A::IDENTITY,
        ));

        Ok(())
    }

    /// Final operation. Produce the [`re_renderer::QueueableDrawData`] to actually be drawn.
    pub fn into_draw_data(
        self,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let Self {
            data: _,
            line_builder,
            line_batch_debug_label: _,
            solid_instances,
            query: _,
            render_ctx,
        } = self;
        let wireframe_draw_data: re_renderer::QueueableDrawData =
            line_builder.into_draw_data()?.into();

        let solid_draw_data: Option<re_renderer::QueueableDrawData> =
            match re_renderer::renderer::MeshDrawData::new(render_ctx, &solid_instances) {
                Ok(draw_data) => Some(draw_data.into()),
                Err(err) => {
                    re_log::error_once!(
                        "Failed to create mesh draw data from mesh instances: {err}"
                    );
                    None
                }
            };

        Ok([solid_draw_data, Some(wireframe_draw_data)]
            .into_iter()
            .flatten()
            .collect())
    }
}
