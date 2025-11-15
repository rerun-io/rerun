use std::iter;

use re_renderer::{BoxCloudBuilder, PickingLayerInstanceId, PickingLayerObjectId};
use re_types::{
    ArrowString,
    archetypes::Boxes3D,
    components::{ClassId, Color, FillMode, HalfSize3D, Radius, ShowLabels},
};
use re_view::process_color_slice;
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, QueryContext, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
};

use crate::{contexts::SpatialSceneEntityContext, proc_mesh, view_kind::SpatialViewKind};

use super::{
    SpatialViewVisualizerData, filter_visualizable_3d_entities, process_labels_3d,
    utilities::{LabeledBatch, ProcMeshBatch, ProcMeshDrawableBuilder},
};

/// Threshold for number of boxes above which we use the fast GPU-accelerated box cloud renderer.
/// Below this threshold, we use the traditional proc mesh renderer for compatibility with wireframes.
const BOX_CLOUD_THRESHOLD: usize = 1000;

// ---
pub struct Boxes3DVisualizer(SpatialViewVisualizerData);

impl Default for Boxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's_a lot_.
impl Boxes3DVisualizer {
    /// Check if a transform is trivial (translation-only, no rotation or non-uniform scaling).
    fn is_transform_trivial(transform: &glam::DAffine3) -> bool {
        // Check if the 3x3 matrix part (rotation/scale) is identity
        let matrix3 = transform.matrix3;
        let identity = glam::DMat3::IDENTITY;

        // Allow small epsilon for floating point comparison
        const EPSILON: f64 = 1e-6;

        for i in 0..3 {
            for j in 0..3 {
                if (matrix3.col(j)[i] - identity.col(j)[i]).abs() > EPSILON {
                    return false;
                }
            }
        }

        true
    }

    /// Fast path for rendering many solid boxes using the GPU-accelerated box cloud renderer.
    /// Only used when all transforms are trivial (translation-only).
    fn process_data_fast_path<'a>(
        data: &mut SpatialViewVisualizerData,
        render_ctx: &re_renderer::RenderContext,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        latest_at: re_log_types::TimeInt,
        batches: impl Iterator<Item = Boxes3DComponentData<'a>>,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let entity_path = query_context.target_entity_path;
        let mut box_builder = BoxCloudBuilder::new(render_ctx);

        // Process all batches
        for batch in batches {
            let num_instances = batch.half_sizes.len();
            if num_instances == 0 {
                continue;
            }

            // Get per-instance transforms from transform tree
            let target_from_poses = ent_context.transform_info.target_from_instances();

            // Calculate actual number of instances (max of all component lengths)
            let num_instances = num_instances
                .max(target_from_poses.len())
                .max(batch.centers.len());

            // Clamp half_sizes to num_instances (repeat last if needed, like slow path does)
            let last_half_size = batch.half_sizes.last().map(|hs| glam::Vec3::from(hs.0)).unwrap_or(glam::Vec3::ONE);
            let half_sizes: Vec<glam::Vec3> = batch
                .half_sizes
                .iter()
                .map(|hs| glam::Vec3::from(hs.0))
                .chain(std::iter::repeat(last_half_size))
                .take(num_instances)
                .collect();

            // Clamp target_from_instances to num_instances (repeat last if needed)
            let last_transform = target_from_poses.last();
            let clamped_transforms: Vec<glam::DAffine3> = target_from_poses
                .iter()
                .chain(std::iter::repeat(last_transform))
                .copied()
                .take(num_instances)
                .collect();

            // Get centers from component data, properly clamped
            let component_centers: Vec<glam::DVec3> = if batch.centers.is_empty() {
                vec![glam::DVec3::ZERO; num_instances]
            } else {
                let last_center = batch.centers.last().map(|c| {
                    let [x, y, z] = c.0 .0;
                    glam::DVec3::new(x as f64, y as f64, z as f64)
                }).unwrap_or(glam::DVec3::ZERO);

                batch.centers
                    .iter()
                    .map(|c| {
                        let [x, y, z] = c.0 .0;
                        glam::DVec3::new(x as f64, y as f64, z as f64)
                    })
                    .chain(std::iter::repeat(last_center))
                    .take(num_instances)
                    .collect()
            };

            // Apply per-instance transform to each center
            let centers: Vec<glam::Vec3> = clamped_transforms
                .iter()
                .zip(component_centers.iter())
                .map(|(transform, center)| {
                    transform.transform_point3(*center).as_vec3()
                })
                .collect();

            // Process colors
            let annotation_infos = re_view::process_annotation_slices(
                latest_at,
                num_instances,
                batch.class_ids,
                &ent_context.annotations,
            );

            let colors = process_color_slice(
                query_context,
                Boxes3D::descriptor_colors().component,
                num_instances,
                &annotation_infos,
                batch.colors,
            );

            // Convert colors to Color32
            let colors: Vec<re_renderer::Color32> = colors
                .iter()
                .map(|c| re_renderer::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), c.a()))
                .collect();

            // Create picking IDs
            let picking_ids: Vec<PickingLayerInstanceId> = (0..num_instances)
                .map(|i| PickingLayerInstanceId(i as u64))
                .collect();

            // Compute world space bounding box
            for (center, half_size) in centers.iter().zip(half_sizes.iter()) {
                let min = *center - *half_size;
                let max = *center + *half_size;
                data.bounding_boxes.push((
                    entity_path.hash(),
                    macaw::BoundingBox::from_min_max(min, max),
                ));
            }

            // Add to box builder
            let mut box_batch = box_builder
                .batch(entity_path.to_string())
                .world_from_obj(glam::Affine3A::IDENTITY)
                .depth_offset(ent_context.depth_offset)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(PickingLayerObjectId(entity_path.hash64()))
                .add_boxes(&centers, &half_sizes, &colors, &picking_ids);

            // Determine if there's any sub-ranges that need extra highlighting.
            for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                let highlighted_box_index =
                    (highlighted_key.get() < num_instances as u64).then_some(highlighted_key.get());
                if let Some(highlighted_box_index) = highlighted_box_index {
                    box_batch = box_batch.push_additional_outline_mask_ids_for_range(
                        highlighted_box_index as u32..highlighted_box_index as u32 + 1,
                        *instance_mask_ids,
                    );
                }
            }

            // Process labels the same way as the slow path
            let world_space_bounding_box = macaw::BoundingBox::from_points(
                centers.iter().zip(half_sizes.iter()).flat_map(|(center, half_size)| {
                    [*center - *half_size, *center + *half_size]
                }),
            );

            // Convert colors once to avoid per-batch allocation
            // Safety: egui::Color32 and re_renderer::Color32 have identical memory layout (both are [u8; 4])
            let egui_colors: &[egui::Color32] = bytemuck::cast_slice(&colors);

            data.ui_labels.extend(process_labels_3d(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: world_space_bounding_box.center(),
                    instance_positions: centers.iter().copied(),
                    labels: &batch.labels,
                    colors: egui_colors,
                    show_labels: batch.show_labels.unwrap_or_else(|| {
                        typed_fallback_for(query_context, Boxes3D::descriptor_show_labels().component)
                    }),
                    annotation_infos: &annotation_infos,
                },
                glam::Affine3A::IDENTITY,
            ));
        }

        Ok(vec![box_builder
            .into_draw_data()
            .map_err(|err| {
                ViewSystemExecutionError::DrawDataCreationError(err.to_string().into())
            })?
            .into()])
    }

    /// Slow path for rendering boxes using the traditional proc mesh renderer.
    /// Used for wireframes and small numbers of boxes.
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        batches: impl Iterator<Item = Boxes3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let proc_mesh_key = proc_mesh::ProcMeshKey::Cube;

        // `ProcMeshKey::Cube` is scaled to side length of 1, i.e. a half-size of 0.5.
        // Therefore, to make scaling by half_size work out to the correct result,
        // apply a factor of 2.
        // TODO(kpreid): Is there any non-historical reason to keep that.
        let constant_instance_transform = glam::Affine3A::from_scale(glam::Vec3::splat(2.0));

        for batch in batches {
            builder.add_batch(
                query_context,
                ent_context,
                Boxes3D::descriptor_colors().component,
                Boxes3D::descriptor_show_labels().component,
                constant_instance_transform,
                ProcMeshBatch {
                    half_sizes: batch.half_sizes,
                    centers: batch.centers,
                    rotation_axis_angles: batch.rotation_axis_angles.as_slice(),
                    quaternions: batch.quaternions,
                    meshes: iter::repeat(proc_mesh_key),
                    fill_modes: iter::repeat(batch.fill_mode),
                    line_radii: batch.radii,
                    colors: batch.colors,
                    labels: &batch.labels,
                    show_labels: batch.show_labels,
                    class_ids: batch.class_ids,
                },
            )?;
        }

        Ok(())
    }
}

// ---

struct Boxes3DComponentData<'a> {
    // Point of views
    half_sizes: &'a [HalfSize3D],

    // Clamped to edge
    centers: &'a [re_types::components::PoseTranslation3D],
    rotation_axis_angles: re_chunk_store::external::re_chunk::ChunkComponentIterItem<re_types::components::PoseRotationAxisAngle>,
    quaternions: &'a [re_types::components::PoseRotationQuat],
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
}

impl IdentifiedViewSystem for Boxes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Boxes3D".into()
    }
}

impl VisualizerSystem for Boxes3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Boxes3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: MaybeVisualizableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        use super::entity_iterator::{iter_slices, process_archetype};

        // First pass: determine if we can use the fast path
        let mut num_boxes_total = 0;
        let mut can_use_fast_path = true;

        process_archetype::<Self, Boxes3D, _>(
            ctx,
            view_query,
            context_systems,
            |_ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_half_size_chunks) =
                    results.get_required_chunks(Boxes3D::descriptor_half_sizes().component)
                else {
                    return Ok(());
                };

                num_boxes_total = all_half_size_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 3]>())
                    .map(|vectors| vectors.len())
                    .sum();

                // Check fill mode
                let all_fill_modes =
                    results.iter_as(_ctx.query.timeline(), Boxes3D::descriptor_fill_mode().component);
                let fill_mode: FillMode = all_fill_modes
                    .slice::<u8>()
                    .next()
                    .and_then(|(_, fill_modes)| {
                        fill_modes.first().copied().and_then(FillMode::from_u8)
                    })
                    .unwrap_or_default();

                // Check if there are per-instance rotations in the component data
                let all_rotation_axis_angles = results.iter_as(
                    _ctx.query.timeline(),
                    Boxes3D::descriptor_rotation_axis_angles().component,
                );
                let all_quaternions = results.iter_as(
                    _ctx.query.timeline(),
                    Boxes3D::descriptor_quaternions().component,
                );

                let has_rotations = all_rotation_axis_angles.component_slow::<re_types::components::PoseRotationAxisAngle>()
                    .next()
                    .is_some()
                    || all_quaternions.slice::<[f32; 4]>()
                        .next()
                        .map_or(false, |(_, quats)| !quats.is_empty());

                // Check if any per-instance transform has non-trivial rotation/scale
                // We need to check ALL transforms that will actually be used for rendering.
                // The renderer will use num_instances = max(half_sizes, transforms, centers),
                // so we must check all transforms that exist, not just the first num_boxes_total.
                let target_from_instances = spatial_ctx.transform_info.target_from_instances();

                // Also need to count centers to know actual instance count
                let all_centers = results.iter_as(_ctx.query.timeline(), Boxes3D::descriptor_centers().component);
                let num_centers: usize = all_centers
                    .slice::<[f32; 3]>()
                    .map(|(_, centers)| centers.len())
                    .sum();

                // Calculate the actual number of instances that will be rendered
                let num_instances_to_render = num_boxes_total
                    .max(target_from_instances.len())
                    .max(num_centers);

                // Check all transforms up to num_instances_to_render
                // (or all transforms if there are fewer)
                let num_transforms_to_check = num_instances_to_render.min(target_from_instances.len());

                let has_non_trivial_transforms = target_from_instances
                    .iter()
                    .take(num_transforms_to_check)
                    .any(|transform| !Self::is_transform_trivial(transform));

                // Fast path only for solid boxes above threshold without rotations
                if fill_mode != FillMode::Solid {
                    can_use_fast_path = false;
                } else if num_boxes_total < BOX_CLOUD_THRESHOLD {
                    can_use_fast_path = false;
                } else if has_rotations {
                    re_log::debug!(
                        "Falling back to slow path due to per-instance rotations"
                    );
                    can_use_fast_path = false;
                } else if has_non_trivial_transforms {
                    re_log::debug!(
                        "Falling back to slow path due to non-trivial per-instance transforms (rotation/scaling)"
                    );
                    can_use_fast_path = false;
                }

                Ok(())
            },
        )?;

        // Use fast path for many solid boxes
        if can_use_fast_path && num_boxes_total >= BOX_CLOUD_THRESHOLD {
            re_log::debug!("Using fast box cloud renderer for {num_boxes_total} boxes");

            let mut draw_data_vec = Vec::new();
            let latest_at = view_query.latest_at;

            process_archetype::<Self, Boxes3D, _>(
                ctx,
                view_query,
                context_systems,
                |ctx, spatial_ctx, results| {
                    use re_view::RangeResultsExt as _;

                    let Some(all_half_size_chunks) =
                        results.get_required_chunks(Boxes3D::descriptor_half_sizes().component)
                    else {
                        return Ok(());
                    };

                    let timeline = ctx.query.timeline();
                    let all_half_sizes_indexed =
                        iter_slices::<[f32; 3]>(&all_half_size_chunks, timeline);
                    let all_centers =
                        results.iter_as(timeline, Boxes3D::descriptor_centers().component);
                    let all_rotation_axis_angles = results.iter_as(
                        timeline,
                        Boxes3D::descriptor_rotation_axis_angles().component,
                    );
                    let all_quaternions =
                        results.iter_as(timeline, Boxes3D::descriptor_quaternions().component);
                    let all_colors = results.iter_as(timeline, Boxes3D::descriptor_colors().component);
                    let all_radii = results.iter_as(timeline, Boxes3D::descriptor_radii().component);
                    let all_labels = results.iter_as(timeline, Boxes3D::descriptor_labels().component);
                    let all_class_ids =
                        results.iter_as(timeline, Boxes3D::descriptor_class_ids().component);
                    let all_show_labels =
                        results.iter_as(timeline, Boxes3D::descriptor_show_labels().component);

                    let all_fill_modes =
                        results.iter_as(timeline, Boxes3D::descriptor_fill_mode().component);
                    let fill_mode: FillMode = all_fill_modes
                        .slice::<u8>()
                        .next()
                        .and_then(|(_, fill_modes)| {
                            fill_modes.first().copied().and_then(FillMode::from_u8)
                        })
                        .unwrap_or_default();

                    let data = re_query::range_zip_1x8(
                        all_half_sizes_indexed,
                        all_centers.slice::<[f32; 3]>(),
                        all_rotation_axis_angles.component_slow::<re_types::components::PoseRotationAxisAngle>(),
                        all_quaternions.slice::<[f32; 4]>(),
                        all_colors.slice::<u32>(),
                        all_radii.slice::<f32>(),
                        all_labels.slice::<String>(),
                        all_class_ids.slice::<u16>(),
                        all_show_labels.slice::<bool>(),
                    )
                    .map(
                        |(_index, half_sizes, centers, rotation_axis_angles, quaternions, colors, radii, labels, class_ids, show_labels)| {
                            Boxes3DComponentData {
                                half_sizes: bytemuck::cast_slice(half_sizes),
                                centers: centers.map_or(&[], bytemuck::cast_slice),
                                rotation_axis_angles: rotation_axis_angles.unwrap_or_default(),
                                quaternions: quaternions.map_or(&[], bytemuck::cast_slice),
                                colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                                radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                                fill_mode,
                                labels: labels.unwrap_or_default(),
                                class_ids: class_ids
                                    .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                                show_labels: show_labels
                                    .map(|b| !b.is_empty() && b.value(0))
                                    .map(Into::into),
                            }
                        },
                    );

                    let result = Self::process_data_fast_path(
                        &mut self.0,
                        ctx.viewer_ctx().render_ctx(),
                        ctx,
                        spatial_ctx,
                        latest_at,
                        data,
                    )?;
                    draw_data_vec.extend(result);
                    Ok(())
                },
            )?;

            return Ok(draw_data_vec);
        }

        // Slow path: use traditional proc mesh renderer
        let mut builder = ProcMeshDrawableBuilder::new(
            &mut self.0,
            ctx.viewer_ctx.render_ctx(),
            view_query,
            "boxes3d",
        );

        process_archetype::<Self, Boxes3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_half_size_chunks) =
                    results.get_required_chunks(Boxes3D::descriptor_half_sizes().component)
                else {
                    return Ok(());
                };

                let num_boxes: usize = all_half_size_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 3]>())
                    .map(|vectors| vectors.len())
                    .sum();
                if num_boxes == 0 {
                    return Ok(());
                }

                let timeline = ctx.query.timeline();
                let all_half_sizes_indexed =
                    iter_slices::<[f32; 3]>(&all_half_size_chunks, timeline);
                let all_centers =
                    results.iter_as(timeline, Boxes3D::descriptor_centers().component);
                let all_rotation_axis_angles = results.iter_as(
                    timeline,
                    Boxes3D::descriptor_rotation_axis_angles().component,
                );
                let all_quaternions =
                    results.iter_as(timeline, Boxes3D::descriptor_quaternions().component);
                let all_colors = results.iter_as(timeline, Boxes3D::descriptor_colors().component);
                let all_radii = results.iter_as(timeline, Boxes3D::descriptor_radii().component);
                let all_labels = results.iter_as(timeline, Boxes3D::descriptor_labels().component);
                let all_class_ids =
                    results.iter_as(timeline, Boxes3D::descriptor_class_ids().component);
                let all_show_labels =
                    results.iter_as(timeline, Boxes3D::descriptor_show_labels().component);

                // Deserialized because it's a union.
                let all_fill_modes =
                    results.iter_as(timeline, Boxes3D::descriptor_fill_mode().component);
                // fill mode is currently a non-repeated component
                let fill_mode: FillMode = all_fill_modes
                    .slice::<u8>()
                    .next()
                    .and_then(|(_, fill_modes)| {
                        fill_modes.first().copied().and_then(FillMode::from_u8)
                    })
                    .unwrap_or_default();

                match fill_mode {
                    FillMode::DenseWireframe | FillMode::MajorWireframe => {
                        // Each box consists of 4 strips with a total of 16 vertices
                        builder.line_builder.reserve_strips(num_boxes * 4)?;
                        builder.line_builder.reserve_vertices(num_boxes * 16)?;
                    }
                    FillMode::Solid => {
                        // No lines.
                    }
                }

                let data = re_query::range_zip_1x8(
                    all_half_sizes_indexed,
                    all_centers.slice::<[f32; 3]>(),
                    all_rotation_axis_angles.component_slow::<re_types::components::PoseRotationAxisAngle>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_colors.slice::<u32>(),
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(_index, half_sizes, centers, rotation_axis_angles, quaternions, colors, radii, labels, class_ids, show_labels)| {
                        Boxes3DComponentData {
                            half_sizes: bytemuck::cast_slice(half_sizes),
                            centers: centers.map_or(&[], bytemuck::cast_slice),
                            rotation_axis_angles: rotation_axis_angles.unwrap_or_default(),
                            quaternions: quaternions.map_or(&[], bytemuck::cast_slice),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            // fill mode is currently a non-repeated component
                            fill_mode,
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
                        }
                    },
                );

                Self::process_data(&mut builder, ctx, spatial_ctx, data)?;

                Ok(())
            },
        )?;

        builder.into_draw_data()
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
