use std::sync::Arc;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_byte_size::SizeBytes as _;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PointCloudBuilder, PositionRadius};
use re_sdk_types::Archetype as _;
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::Points3D;
use re_sdk_types::components::{ClassId, Color, KeypointId, Position3D, Radius, ShowLabels};
use re_view::{process_annotation_and_keypoint_slices, process_color_slice};
use re_viewer_context::{
    Cache, IdentifiedViewSystem, QueryContext, ResolvedAnnotationInfos, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput,
    VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
};

use super::utilities::LabeledBatch;
use super::{Keypoints, SpatialViewVisualizerData, process_labels_3d};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::{load_keypoint_connections, process_radius_slice};

// ---

pub struct Points3DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Points3DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::ThreeD)),
        }
    }
}

struct Points3DComponentData<'a> {
    query_result_hash: Hash64,

    // Point of views
    positions: &'a [Position3D],

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

/// Processed/computed point cloud data ready for rendering.
///
/// This bundles together the results of processing raw component data
/// (computing annotations, colors, radii, bounding boxes, etc.)
/// so that it can be memoized based on `data.query_hash`.
struct Points3DCpu {
    position_radii: Vec<PositionRadius>,
    point_cloud_bounds: re_renderer::util::PointCloudBounds,
    picking_ids: Vec<PickingLayerInstanceId>,
    annotation_infos: ResolvedAnnotationInfos,
    keypoints: Keypoints,
    colors: Vec<egui::Color32>,
}

impl Points3DCpu {
    fn compute(
        ctx: &QueryContext<'_>,
        entity_path: &re_log_types::EntityPath,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: &Points3DComponentData<'_>,
    ) -> Self {
        let num_instances = data.positions.len();
        re_tracing::profile_function!(num_instances.to_string());

        let picking_ids = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "picking_ids");
            (0..num_instances)
                .map(|i| PickingLayerInstanceId(i as _))
                .collect_vec()
        };
        let (annotation_infos, keypoints) = process_annotation_and_keypoint_slices(
            query.latest_at,
            num_instances,
            data.positions.iter().map(|p| p.0.into()),
            data.keypoint_ids,
            data.class_ids,
            &ent_context.annotations,
        );

        let positions: &[glam::Vec3] = bytemuck::cast_slice(data.positions);

        let point_cloud_bounds = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "bounding_box");
            re_renderer::util::point_cloud_bounds(positions)
        };

        let radii = process_radius_slice(
            ctx,
            entity_path,
            num_instances,
            data.radii,
            Points3D::descriptor_radii().component,
        );
        let colors = process_color_slice(
            ctx,
            Points3D::descriptor_colors().component,
            num_instances,
            &annotation_infos,
            data.colors,
        );

        let position_radii = PositionRadius::from_many(positions, &radii);

        Self {
            position_radii,
            point_cloud_bounds,
            picking_ids,
            annotation_infos,
            keypoints,
            colors,
        }
    }

    fn heap_size_bytes(&self) -> u64 {
        let Self {
            position_radii,
            point_cloud_bounds: _,
            picking_ids,
            annotation_infos,
            keypoints,
            colors,
        } = self;

        (position_radii.capacity() * std::mem::size_of::<PositionRadius>()) as u64
            + picking_ids.heap_size_bytes()
            + annotation_infos.heap_size_bytes()
            + keypoints.heap_size_bytes()
            + colors.heap_size_bytes()
    }
}

// --- Points3DCache ---

/// All the inputs that affect the output of [`Points3DCpu::compute`],
/// beyond the point data itself (which is covered by `query_result_hash`).
struct Points3DCacheKey {
    /// Hash of the query results (positions, colors, radii, `class_ids`, etc.).
    query_result_hash: Hash64,

    /// The [`super::Annotations::row_id`] of the resolved annotation context.
    /// Changes when the annotation context is re-logged.
    annotation_row_id: re_chunk_store::RowId,
}

impl Points3DCacheKey {
    fn hash(&self) -> Hash64 {
        let Self {
            query_result_hash,
            annotation_row_id,
        } = self;
        Hash64::hash((query_result_hash, annotation_row_id))
    }
}

struct Points3DCacheEntry {
    cpu: Arc<Points3DCpu>,
    last_used_generation: u64,
}

/// Caches [`Points3DCpu`] to avoid recomputing annotations, colors, radii, etc. every frame.
#[derive(Default)]
pub struct Points3DCache {
    cache: IntMap<Hash64, Points3DCacheEntry>,
    generation: u64,
}

impl Points3DCache {
    fn entry(
        &mut self,
        key: &Points3DCacheKey,
        compute: impl FnOnce() -> Points3DCpu,
    ) -> Arc<Points3DCpu> {
        let hash = key.hash();
        let entry = self
            .cache
            .entry(hash)
            .or_insert_with(|| Points3DCacheEntry {
                cpu: Arc::new(compute()),
                last_used_generation: 0,
            });
        entry.last_used_generation = self.generation;
        entry.cpu.clone()
    }
}

impl Cache for Points3DCache {
    fn name(&self) -> &'static str {
        "Points3DCache"
    }

    fn begin_frame(&mut self) {
        self.cache
            .retain(|_, entry| entry.last_used_generation == self.generation);
        self.generation += 1;
    }

    fn purge_memory(&mut self) {
        self.cache.clear();
    }

    fn on_store_events(
        &mut self,
        _events: &[&re_chunk_store::ChunkStoreEvent],
        _entity_db: &EntityDb,
    ) {
    }
}

impl re_byte_size::SizeBytes for Points3DCache {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache,
            generation: _,
        } = self;
        // Count the underlying data of the Arc directly instead of weighing active
        cache
            .values()
            .map(|entry| entry.cpu.heap_size_bytes() + std::mem::size_of_val(&entry.cpu) as u64)
            .sum::<u64>()
            + (cache.capacity() * std::mem::size_of::<(Hash64, Points3DCacheEntry)>()) as u64
    }
}

impl re_byte_size::MemUsageTreeCapture for Points3DCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.total_size_bytes())
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Points3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        point_builder: &mut PointCloudBuilder<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: impl Iterator<Item = Points3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.positions.len();
            if num_instances == 0 {
                continue;
            }

            let cache_key = Points3DCacheKey {
                query_result_hash: data.query_result_hash,
                annotation_row_id: ent_context.annotations.row_id(),
            };

            let cpu = ctx.store_ctx().memoizer(|c: &mut Points3DCache| {
                c.entry(&cache_key, || {
                    Points3DCpu::compute(ctx, entity_path, query, ent_context, &data)
                })
            });

            // TODO(grtlr): The following is a quick fix to get multiple instance poses to work
            // with point clouds: We sent the same point cloud multiple times to the GPU (bad
            // for memory) and render them with multiple draw calls across different batches (bad
            // for performance).
            for world_from_obj in ent_context
                .transform_info
                .target_from_instances()
                .iter()
                .map(|transform| transform.as_affine3a())
            {
                re_tracing::profile_scope!("one-transform");

                let point_batch = point_builder
                    .batch(entity_path.to_string())
                    .world_from_obj(world_from_obj)
                    .outline_mask_ids(ent_context.highlight.overall)
                    .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

                let mut point_range_builder =
                    point_batch.add_points(&cpu.position_radii, &cpu.colors, &cpu.picking_ids);

                // Determine if there's any sub-ranges that need extra highlighting.
                {
                    for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                        let highlighted_point_index = (highlighted_key.get()
                            < num_instances as u64)
                            .then_some(highlighted_key.get());
                        if let Some(highlighted_point_index) = highlighted_point_index {
                            point_range_builder = point_range_builder
                                .push_additional_outline_mask_ids_for_range(
                                    highlighted_point_index as u32
                                        ..highlighted_point_index as u32 + 1,
                                    *instance_mask_ids,
                                );
                        }
                    }
                }

                self.data.add_bounding_box_and_region_of_interest(
                    entity_path.hash(),
                    cpu.point_cloud_bounds.bbox,
                    cpu.point_cloud_bounds.region_of_interest,
                    world_from_obj,
                );

                load_keypoint_connections(
                    line_builder,
                    &ent_context.annotations,
                    world_from_obj,
                    entity_path,
                    &cpu.keypoints,
                )?;

                self.data.ui_labels.extend(process_labels_3d(
                    LabeledBatch {
                        entity_path,
                        visualizer_instruction: ent_context.visualizer_instruction,
                        num_instances,
                        overall_position: cpu.point_cloud_bounds.bbox.center(),
                        instance_positions: cpu.position_radii.iter().map(|pr| pr.pos),
                        labels: &data.labels,
                        colors: &cpu.colors,
                        show_labels: data.show_labels.unwrap_or_else(|| {
                            typed_fallback_for(ctx, Points3D::descriptor_show_labels().component)
                        }),
                        annotation_infos: &cpu.annotation_infos,
                    },
                    world_from_obj,
                ));
            }
        }

        Ok(())
    }
}

impl IdentifiedViewSystem for Points3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Points3D".into()
    }
}

impl VisualizerSystem for Points3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Position3D>(
            &Points3D::descriptor_positions(),
            &Points3D::all_components(),
        )
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();
        let output = VisualizerExecutionOutput::default();

        let mut point_builder = PointCloudBuilder::new(ctx.viewer_ctx.render_ctx());
        point_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        // We need lines from keypoints. The number of lines we'll have is harder to predict, so we'll go
        // with the dynamic allocation approach.
        let mut line_builder = LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Self, Points3D, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                re_tracing::profile_scope!("Point3D");

                let all_positions =
                    results.iter_required(Points3D::descriptor_positions().component);
                if all_positions.is_empty() {
                    return Ok(());
                }

                let num_positions = {
                    re_tracing::profile_scope!("num_positions");
                    all_positions
                        .chunks()
                        .iter()
                        .flat_map(|chunk| chunk.iter_slices::<[f32; 3]>())
                        .map(|points| points.len())
                        .sum()
                };

                if num_positions == 0 {
                    return Ok(());
                }

                point_builder.reserve(num_positions)?;
                let all_colors = results.iter_optional(Points3D::descriptor_colors().component);
                let all_radii = results.iter_optional(Points3D::descriptor_radii().component);
                let all_labels = results.iter_optional(Points3D::descriptor_labels().component);
                let all_class_ids =
                    results.iter_optional(Points3D::descriptor_class_ids().component);
                let all_keypoint_ids =
                    results.iter_optional(Points3D::descriptor_keypoint_ids().component);
                let all_show_labels =
                    results.iter_optional(Points3D::descriptor_show_labels().component);

                let query_result_hash = results.query_result_hash();

                let data = re_query::range_zip_1x6(
                    all_positions.slice::<[f32; 3]>(), // RowId 5
                    all_colors.slice::<u32>(),         // RowId 7
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_keypoint_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(
                        _index,
                        positions,
                        colors,
                        radii,
                        labels,
                        class_ids,
                        keypoint_ids,
                        show_labels,
                    )| {
                        Points3DComponentData {
                            query_result_hash,
                            positions: bytemuck::cast_slice(positions),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            keypoint_ids: keypoint_ids
                                .map_or(&[], |keypoint_ids| bytemuck::cast_slice(keypoint_ids)),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
                        }
                    },
                );

                self.process_data(
                    ctx,
                    &mut point_builder,
                    &mut line_builder,
                    view_query,
                    spatial_ctx,
                    data,
                )
            },
        )?;

        Ok(output.with_draw_data([
            point_builder.into_draw_data()?.into(),
            line_builder.into_draw_data()?.into(),
        ]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}
