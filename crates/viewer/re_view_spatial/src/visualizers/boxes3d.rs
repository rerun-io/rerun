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

// Test-only counter to verify fast path is actually used
#[cfg(test)]
static FAST_PATH_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub fn reset_fast_path_counter() {
    FAST_PATH_COUNTER.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
pub fn get_fast_path_count() -> usize {
    FAST_PATH_COUNTER.load(std::sync::atomic::Ordering::Relaxed);
}

// ---
pub struct Boxes3DVisualizer(SpatialViewVisualizerData);

impl Default for Boxes3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

/// Common data extracted from Boxes3D components, used by both rendering paths.
struct ProcessedBoxData {
    centers: Vec<glam::Vec3>,
    half_sizes: Vec<glam::Vec3>,
    colors: Vec<re_renderer::Color32>,
    picking_ids: Vec<PickingLayerInstanceId>,
    annotation_infos: Vec<re_view::AnnotationInfo>,
    labels: Vec<ArrowString>,
    show_labels: ShowLabels,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's a lot.
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

    /// Extract and process box data from components.
    /// This is shared logic used by both fast and slow rendering paths.
    fn process_box_data<'a>(
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        latest_at: re_log_types::TimeInt,
        batch: Boxes3DComponentData<'a>,
    ) -> ProcessedBoxData {
        let num_instances = batch.half_sizes.len();
        if num_instances == 0 {
            return ProcessedBoxData {
                centers: Vec::new(),
                half_sizes: Vec::new(),
                colors: Vec::new(),
                picking_ids: Vec::new(),
                annotation_infos: Vec::new(),
                labels: Vec::new(),
                show_labels: ShowLabels(false),
            };
        }

        // Get per-instance transforms from transform tree
        let target_from_poses = ent_context.transform_info.target_from_instances();

        // Calculate actual number of instances (max of all component lengths)
        let num_instances = num_instances
            .max(target_from_poses.len())
            .max(batch.centers.len());

        // Clamp half_sizes to num_instances (repeat last if needed)
        let last_half_size = batch
            .half_sizes
            .last()
            .map(|hs| glam::Vec3::from(hs.0))
            .unwrap_or(glam::Vec3::ONE);
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
            let last_center = batch
                .centers
                .last()
                .map(|c| {
                    let [x, y, z] = c.0 .0;
                    glam::DVec3::new(x as f64, y as f64, z as f64)
                })
                .unwrap_or(glam::DVec3::ZERO);

            batch
                .centers
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
            .map(|(transform, center)| transform.transform_point3(*center).as_vec3())
            .collect();

        // Process colors with annotations
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
            .map(|c| {
                re_renderer::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), c.a())
            })
            .collect();

        // Create picking IDs
        let picking_ids: Vec<PickingLayerInstanceId> = (0..num_instances)
            .map(|i| PickingLayerInstanceId(i as u64))
            .collect();

        let show_labels = batch.show_labels.unwrap_or_else(|| {
            typed_fallback_for(query_context, Boxes3D::descriptor_show_labels().component)
        });

        ProcessedBoxData {
            centers,
            half_sizes,
            colors,
            picking_ids,
            annotation_infos,
            labels: batch.labels,
            show_labels,
        }
    }

    /// Fast path for rendering solid boxes using the GPU-accelerated box cloud renderer.
    /// Used when all transforms are trivial (translation-only) and fill mode is solid.
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

        // Track bounding box for this entity
        let mut entity_bbox = macaw::BoundingBox::nothing();

        // Process all batches
        for batch in batches {
            let processed = Self::process_box_data(query_context, ent_context, latest_at, batch);

            if processed.centers.is_empty() {
                continue;
            }

            let num_instances = processed.centers.len();

            // Compute bounding box for this batch
            for (center, half_size) in processed.centers.iter().zip(processed.half_sizes.iter()) {
                let min = *center - *half_size;
                let max = *center + *half_size;
                entity_bbox = entity_bbox.union(macaw::BoundingBox::from_min_max(min, max));
            }

            // Add to box builder
            let mut box_batch = box_builder
                .batch(entity_path.to_string())
                .world_from_obj(glam::Affine3A::IDENTITY)
                .depth_offset(ent_context.depth_offset)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(PickingLayerObjectId(entity_path.hash64()))
                .add_boxes(
                    &processed.centers,
                    &processed.half_sizes,
                    &processed.colors,
                    &processed.picking_ids,
                );

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

            // Process labels
            let world_space_bounding_box = macaw::BoundingBox::from_points(
                processed
                    .centers
                    .iter()
                    .zip(processed.half_sizes.iter())
                    .flat_map(|(center, half_size)| [*center - *half_size, *center + *half_size]),
            );

            // Convert colors for labels
            // Safety: egui::Color32 and re_renderer::Color32 have identical memory layout
            let egui_colors: &[egui::Color32] = bytemuck::cast_slice(&processed.colors);

            data.ui_labels.extend(process_labels_3d(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: world_space_bounding_box.center(),
                    instance_positions: processed.centers.iter().copied(),
                    labels: &processed.labels,
                    colors: egui_colors,
                    show_labels: processed.show_labels,
                    annotation_infos: &processed.annotation_infos,
                },
                glam::Affine3A::IDENTITY,
            ));
        }

        // Add single union bounding box for the entire entity
        if entity_bbox.is_something() && entity_bbox.is_finite() {
            data.add_bounding_box(entity_path.hash(), entity_bbox, glam::Affine3A::IDENTITY);
        }

        Ok(vec![box_builder
            .into_draw_data()
            .map_err(|err| {
                ViewSystemExecutionError::DrawDataCreationError(err.to_string().into())
            })?
            .into()])
    }

    /// Slow path for rendering boxes using the traditional proc mesh renderer.
    /// Used for wireframes or when transforms include rotation/scaling.
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        batches: impl Iterator<Item = Boxes3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let proc_mesh_key = proc_mesh::ProcMeshKey::Cube;

        // ProcMeshKey::Cube is scaled to side length of 1, i.e. a half-size of 0.5.
        // Therefore, to make scaling by half_size work out to the correct result,
        // apply a factor of 2.
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
    rotation_axis_angles: re_chunk_store::external::re_chunk::ChunkComponentIterItem<
        re_types::components::PoseRotationAxisAngle,
    >,
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

        // Determine if we can use the fast path by checking eligibility criteria
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

                let num_boxes_total: usize = all_half_size_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 3]>())
                    .map(|vectors| vectors.len())
                    .sum();

                if num_boxes_total == 0 {
                    can_use_fast_path = false;
                    return Ok(());
                }

                // Check fill mode - fast path only supports solid boxes
                let all_fill_modes = results
                    .iter_as(_ctx.query.timeline(), Boxes3D::descriptor_fill_mode().component);
                let fill_mode: FillMode = all_fill_modes
                    .slice::<u8>()
                    .next()
                    .and_then(|(_, fill_modes)| {
                        fill_modes.first().copied().and_then(FillMode::from_u8)
                    })
                    .unwrap_or_default();

                if fill_mode != FillMode::Solid {
                    can_use_fast_path = false;
                    return Ok(());
                }

                // Check if there are per-instance rotations in the component data
                let all_rotation_axis_angles = results.iter_as(
                    _ctx.query.timeline(),
                    Boxes3D::descriptor_rotation_axis_angles().component,
                );
                let all_quaternions = results.iter_as(
                    _ctx.query.timeline(),
                    Boxes3D::descriptor_quaternions().component,
                );

                let has_rotations = all_rotation_axis_angles
                    .component_slow::<re_types::components::PoseRotationAxisAngle>()
                    .next()
                    .is_some()
                    || all_quaternions
                        .slice::<[f32; 4]>()
                        .next()
                        .map_or(false, |(_, quats)| !quats.is_empty());

                if has_rotations {
                    re_log::debug!(
                        "Boxes3D: using slow path due to per-instance rotations"
                    );
                    can_use_fast_path = false;
                    return Ok(());
                }

                // Check if any per-instance transform has non-trivial rotation/scale
                let target_from_instances = spatial_ctx.transform_info.target_from_instances();

                // Count centers to determine actual instance count
                let all_centers =
                    results.iter_as(_ctx.query.timeline(), Boxes3D::descriptor_centers().component);
                let num_centers: usize = all_centers
                    .slice::<[f32; 3]>()
                    .map(|(_, centers)| centers.len())
                    .sum();

                // Calculate the actual number of instances that will be rendered
                let num_instances_to_render = num_boxes_total
                    .max(target_from_instances.len())
                    .max(num_centers);

                // Check all transforms that will be used
                let num_transforms_to_check =
                    num_instances_to_render.min(target_from_instances.len());

                let has_non_trivial_transforms = target_from_instances
                    .iter()
                    .take(num_transforms_to_check)
                    .any(|transform| !Self::is_transform_trivial(transform));

                if has_non_trivial_transforms {
                    re_log::debug!(
                        "Boxes3D: using slow path due to non-trivial transforms (rotation/scaling)"
                    );
                    can_use_fast_path = false;
                }

                Ok(())
            },
        )?;

        // Use fast path when eligible (solid boxes, no rotations, translation-only transforms)
        if can_use_fast_path {
            re_log::debug!("Boxes3D: using fast instanced box cloud renderer");

            #[cfg(test)]
            FAST_PATH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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
                    let all_colors =
                        results.iter_as(timeline, Boxes3D::descriptor_colors().component);
                    let all_radii =
                        results.iter_as(timeline, Boxes3D::descriptor_radii().component);
                    let all_labels =
                        results.iter_as(timeline, Boxes3D::descriptor_labels().component);
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
                        all_rotation_axis_angles
                            .component_slow::<re_types::components::PoseRotationAxisAngle>(),
                        all_quaternions.slice::<[f32; 4]>(),
                        all_colors.slice::<u32>(),
                        all_radii.slice::<f32>(),
                        all_labels.slice::<String>(),
                        all_class_ids.slice::<u16>(),
                        all_show_labels.slice::<bool>(),
                    )
                    .map(
                        |(
                            _index,
                            half_sizes,
                            centers,
                            rotation_axis_angles,
                            quaternions,
                            colors,
                            radii,
                            labels,
                            class_ids,
                            show_labels,
                        )| {
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
        re_log::debug!("Boxes3D: using slow proc mesh renderer");

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

                let all_fill_modes =
                    results.iter_as(timeline, Boxes3D::descriptor_fill_mode().component);
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
                    all_rotation_axis_angles
                        .component_slow::<re_types::components::PoseRotationAxisAngle>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_colors.slice::<u32>(),
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(
                        _index,
                        half_sizes,
                        centers,
                        rotation_axis_angles,
                        quaternions,
                        colors,
                        radii,
                        labels,
                        class_ids,
                        show_labels,
                    )| {
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
