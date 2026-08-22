use std::sync::{Arc, OnceLock};

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use parking_lot::Mutex;
use re_byte_size::SizeBytes as _;
use re_entity_db::EntityDb;
use re_log::ResultExt as _;
use re_log_types::hash::Hash64;
use re_renderer::renderer::{
    GpuPointCloud, PointCloudBatchFlags, PointCloudBatchInfo, PointCloudDrawData,
};
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PositionRadius, SortOrderCache};
use re_sdk_types::Archetype as _;
use re_sdk_types::ArrowString;
use re_sdk_types::archetypes::Points3D;
use re_sdk_types::components::{
    ClassId, Color, KeypointId, PointShading, Position3D, Radius, ShowLabels,
};
use re_sdk_types::reflection::Enum as _;
use re_view::{process_annotation_and_keypoint_slices, process_color_slice};
use re_viewer_context::{
    Cache, IdentifiedViewSystem, QueryContext, ResolvedAnnotationInfos, ViewClass as _,
    ViewContext, ViewContextCollection, ViewQuery, ViewSystemExecutionError,
    VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem, typed_fallback_for,
};

use super::utilities::LabeledBatch;
use super::{Keypoints, SpatialViewVisualizerData, process_labels_3d};
use crate::SpaceKind;
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::visualizers::{load_keypoint_connections, process_radius_slice};

// ---

#[derive(Default)]
pub struct Points3DVisualizer;

struct Points3DComponentData<'a> {
    index: (re_log_types::TimeInt, re_chunk_store::RowId),
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
    point_shading: Option<PointShading>,
}

/// Processed/computed point cloud data ready for rendering.
///
/// This bundles together the results of processing raw component data
/// (computing annotations, colors, radii, bounding boxes, etc.)
/// so that it can be memoized based on `data.query_hash`.
#[derive(re_byte_size::SizeBytes)]
struct Points3DCpu {
    position_radii: Vec<PositionRadius>,

    #[size_bytes(ignore)] // Lives entirely on the stack.
    robust_bounds: re_renderer::RobustBounds,

    picking_ids: Vec<PickingLayerInstanceId>,
    annotation_infos: ResolvedAnnotationInfos,
    keypoints: Keypoints,
    colors: Vec<egui::Color32>,

    /// Whether any point has a non-opaque color, requiring alpha-blended rendering.
    has_transparency: bool,

    /// Scratch buffers holding the back-to-front point ordering across frames.
    ///
    /// Each instance transform has its own cache, which tracks ordering per rendered view.
    sort_order_caches: Mutex<Vec<SortOrderCache>>,

    /// The same point data residing on the GPU, uploaded once when this entry is created.
    ///
    /// `None` if there was nothing to upload or the upload failed.
    #[size_bytes(ignore)] // VRAM, reported through `Points3DCache::vram_usage` instead.
    gpu: Option<Arc<GpuPointCloud>>,

    /// Object-space positions handed to the renderer for back-to-front sorting.
    ///
    /// Only materialized for clouds that are actually drawn transparently.
    #[size_bytes(ignore)] // Only populated for transparent clouds.
    sort_positions: OnceLock<Arc<Vec<glam::Vec3>>>,
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

        let robust_bounds = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "bounding_box");
            re_renderer::RobustBounds::from_points(positions)
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

        let has_transparency = colors.iter().any(|c| !c.is_opaque());

        // Uploading here (rather than per frame) is the whole point of this cache: the same
        // textures are reused for as long as this entry lives.
        let gpu = GpuPointCloud::new(
            ctx.viewer_ctx().render_ctx(),
            &position_radii,
            &colors,
            &picking_ids,
        )
        .ok_or_log_error()
        .flatten()
        .map(Arc::new);

        Self {
            position_radii,
            robust_bounds,
            picking_ids,
            annotation_infos,
            keypoints,
            colors,
            has_transparency,
            sort_order_caches: Mutex::new(Vec::new()),
            gpu,
            sort_positions: OnceLock::new(),
        }
    }

    fn sort_order_cache(&self, transform_index: usize) -> SortOrderCache {
        let mut caches = self.sort_order_caches.lock();
        caches.resize_with(transform_index + 1, SortOrderCache::default);
        caches[transform_index].clone()
    }

    fn sort_positions(&self) -> Arc<Vec<glam::Vec3>> {
        self.sort_positions
            .get_or_init(|| {
                re_tracing::profile_scope!("sort_positions");
                Arc::new(self.position_radii.iter().map(|pr| pr.pos).collect())
            })
            .clone()
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

    fn vram_usage(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(
            self.cache
                .values()
                .filter_map(|entry| entry.cpu.gpu.as_ref())
                .map(|gpu| gpu.gpu_byte_size())
                .sum(),
        )
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
            .map(|entry| {
                entry.cpu.as_ref().heap_size_bytes() + std::mem::size_of_val(&entry.cpu) as u64
            })
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
        view_data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        point_draw_data: &mut Vec<PointCloudDrawData>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: impl Iterator<Item = Points3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();
        let entity_path = ctx.target_entity_path;

        // Opt-in due to the cost of CPU-sorting transparent point clouds every frame.
        let transparency_enabled = ctx
            .viewer_ctx()
            .app_options()
            .experimental
            .point_cloud_transparency;

        for data in data {
            let num_instances = data.positions.len();
            if num_instances == 0 {
                continue;
            }

            let cache_key = Points3DCacheKey {
                query_result_hash: Hash64::hash((data.query_result_hash, data.index)),
                annotation_row_id: ent_context.annotations.row_id(),
            };

            let cpu = ctx.store_ctx().memoizer(|c: &mut Points3DCache| {
                c.entry(&cache_key, || {
                    Points3DCpu::compute(ctx, entity_path, query, ent_context, &data)
                })
            });
            let point_shading = data.point_shading.unwrap_or_else(|| {
                typed_fallback_for(ctx, Points3D::descriptor_point_shading().component)
            });

            // All instance transforms draw the same, singly-uploaded point data; only the
            // per-batch state differs.
            let Some(gpu_cloud) = cpu.gpu.clone() else {
                continue;
            };
            let point_count = gpu_cloud.num_points();
            let mut batches =
                Vec::with_capacity(ent_context.transform_info.target_from_instances().len());

            for (transform_index, world_from_obj) in ent_context
                .transform_info
                .target_from_instances()
                .iter()
                .map(|transform| transform.as_affine3a())
                .enumerate()
            {
                re_tracing::profile_scope!("one-transform");

                let alpha_blend = transparency_enabled && cpu.has_transparency;

                let mut flags = PointCloudBatchFlags::empty();
                flags.set(
                    PointCloudBatchFlags::FLAG_ENABLE_SHADING,
                    matches!(point_shading, PointShading::Gradient),
                );
                flags.set(PointCloudBatchFlags::FLAG_PREMULTIPLIED_ALPHA, alpha_blend);

                // Determine if there's any sub-ranges that need extra highlighting.
                let mut additional_outline_mask_ids_vertex_ranges = Vec::new();
                {
                    #[expect(clippy::iter_over_hash_type)]
                    // Non-overlapping per-instance mask ranges.
                    for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                        let highlighted_point_index = (highlighted_key.get()
                            < num_instances as u64)
                            .then_some(highlighted_key.get());
                        if let Some(highlighted_point_index) = highlighted_point_index {
                            additional_outline_mask_ids_vertex_ranges.push((
                                highlighted_point_index as u32..highlighted_point_index as u32 + 1,
                                *instance_mask_ids,
                            ));
                        }
                    }
                }

                batches.push(PointCloudBatchInfo {
                    label: entity_path.to_string().into(),
                    world_from_obj,
                    flags,
                    point_count,
                    // Every transform draws the entire, shared upload.
                    first_point_index: Some(0),
                    object_space_bounding_box: cpu.robust_bounds.exact,
                    overall_outline_mask_ids: ent_context.highlight.overall,
                    additional_outline_mask_ids_vertex_ranges,
                    picking_object_id: re_renderer::PickingLayerObjectId(entity_path.hash64()),
                    depth_offset: 0,
                    sort_positions: alpha_blend.then(|| cpu.sort_positions()),
                    sort_order_cache: alpha_blend.then(|| cpu.sort_order_cache(transform_index)),
                });

                view_data.add_bounds(
                    entity_path.hash(),
                    cpu.robust_bounds,
                    world_from_obj,
                    SpaceKind::ThreeD,
                );

                load_keypoint_connections(
                    line_builder,
                    &ent_context.annotations,
                    world_from_obj,
                    entity_path,
                    &cpu.keypoints,
                )?;

                view_data.ui_labels.extend(process_labels_3d(
                    LabeledBatch {
                        entity_path,
                        visualizer_instruction: ent_context.visualizer_instruction,
                        num_instances,
                        overall_position: cpu.robust_bounds.exact.center(),
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

            point_draw_data.push(PointCloudDrawData::from_gpu_cloud(
                ctx.viewer_ctx().render_ctx(),
                &gpu_cloud,
                &batches,
                re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
            ));
        }

        Ok(())
    }
}

impl IdentifiedViewSystem for Points3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Points3D"
        )
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

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        Some(crate::SpatialView3D::identifier())
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();
        let mut view_data = SpatialViewVisualizerData::default();
        let output = VisualizerExecutionOutput::default();

        // One draw data per cached point cloud: each has its own, independently cached upload.
        let mut point_draw_data = Vec::new();

        // We need lines from keypoints. The number of lines we'll have is harder to predict, so we'll go
        // with the dynamic allocation approach.
        let mut line_builder = LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Points3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                re_tracing::profile_scope!("Point3D");

                let all_positions =
                    results.iter_required(Points3D::descriptor_positions().component);
                if all_positions.is_empty() {
                    return Ok(());
                }

                let num_positions: usize = {
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

                let all_colors = results.iter_optional(Points3D::descriptor_colors().component);
                let all_radii = results.iter_optional(Points3D::descriptor_radii().component);
                let all_labels = results.iter_optional(Points3D::descriptor_labels().component);
                let all_class_ids =
                    results.iter_optional(Points3D::descriptor_class_ids().component);
                let all_keypoint_ids =
                    results.iter_optional(Points3D::descriptor_keypoint_ids().component);
                let all_show_labels =
                    results.iter_optional(Points3D::descriptor_show_labels().component);
                let all_point_shading =
                    results.iter_optional(Points3D::descriptor_point_shading().component);

                let query_result_hash = results.query_result_hash();

                let results_iter = re_query::range_zip_1x7(
                    all_positions.slice::<[f32; 3]>(), // RowId 5
                    all_colors.slice::<u32>(),         // RowId 7
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_keypoint_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                    all_point_shading.slice::<u8>(),
                )
                .map(
                    |(
                        index,
                        positions,
                        colors,
                        radii,
                        labels,
                        class_ids,
                        keypoint_ids,
                        show_labels,
                        point_shading,
                    )| {
                        Points3DComponentData {
                            index,
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
                            point_shading: point_shading
                                .and_then(|s| PointShading::from_integer_slice(s).next()?),
                        }
                    },
                );

                Self::process_data(
                    &mut view_data,
                    ctx,
                    &mut point_draw_data,
                    &mut line_builder,
                    view_query,
                    spatial_ctx,
                    results_iter,
                )
            },
        )?;

        Ok(output
            .with_draw_data(itertools::chain!(
                point_draw_data.into_iter().map(Into::into),
                [line_builder.into_draw_data()?.into()],
            ))
            .with_visualizer_data(view_data))
    }
}
