//! Placeholder gaussian visualizer until we have a proper one.

use std::sync::Arc;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_byte_size::SizeBytes as _;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_renderer::{PickingLayerInstanceId, PointCloudBuilder, PositionRadius};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::GaussianSplats3D;
use re_sdk_types::components::{Color, Position3D, Scale3D};
use re_view::clamped_or_nothing;
use re_viewer_context::{
    Cache, IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use crate::SpaceKind;
use crate::contexts::SpatialSceneVisualizerInstructionContext;

// ---

/// Renders [`GaussianSplats3D`] via the point cloud renderer.
///
/// This is a first step: each gaussian is drawn as an opaque point whose radius is the geometric
/// mean of the gaussian's per-axis scales. Orientation, opacity, and view-dependent color
/// (spherical harmonics) are all ignored until we have a real splat renderer.
#[derive(Default)]
pub struct GaussianSplats3DVisualizer;

struct GaussianSplats3DComponentData<'a> {
    index: (re_log_types::TimeInt, re_chunk_store::RowId),
    query_result_hash: Hash64,

    // Point of views
    centers: &'a [Position3D],

    // Clamped to edge
    scales: &'a [Scale3D],
    colors: &'a [Color],
}

/// Processed/computed gaussian cloud data ready for rendering.
///
/// This bundles together the results of processing raw component data
/// (computing colors, radii, bounding boxes, etc.)
/// so that it can be memoized based on `data.query_hash`.
struct GaussianSplats3DCpu {
    position_radii: Vec<PositionRadius>,
    point_cloud_bounds: re_renderer::util::PointCloudBounds,
    picking_ids: Vec<PickingLayerInstanceId>,
    colors: Vec<egui::Color32>,
}

impl GaussianSplats3DCpu {
    fn compute(ctx: &QueryContext<'_>, data: &GaussianSplats3DComponentData<'_>) -> Self {
        let num_instances = data.centers.len();
        re_tracing::profile_function!(num_instances.to_string());

        let picking_ids = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "picking_ids");
            (0..num_instances)
                .map(|i| PickingLayerInstanceId(i as _))
                .collect_vec()
        };

        let positions: &[glam::Vec3] = bytemuck::cast_slice(data.centers);

        let point_cloud_bounds = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "bounding_box");
            re_renderer::util::point_cloud_bounds(positions)
        };

        let radii = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "radii");
            if data.scales.is_empty() {
                vec![re_renderer::Size::ONE_UI_POINT; num_instances]
            } else {
                data.scales
                    .iter()
                    .map(|scale| {
                        // Collapse the anisotropic scale to a single radius (the geometric mean).
                        let [x, y, z]: [f32; 3] = scale.0.0;
                        re_renderer::Size::new_scene_units((x * y * z).cbrt())
                    })
                    .collect_vec()
            }
        };

        let colors = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "colors");
            // Gaussians have no class ids, so there is no annotation context to consult:
            // it's the logged colors, or the fallback.
            if data.colors.is_empty() {
                let fallback = typed_fallback_for::<Color>(
                    ctx,
                    GaussianSplats3D::descriptor_colors().component,
                );
                vec![egui::Color32::from(fallback); num_instances]
            } else {
                clamped_or_nothing(data.colors, num_instances)
                    .map(|color| egui::Color32::from(*color))
                    .collect()
            }
        };

        let position_radii = PositionRadius::from_many(positions, &radii);

        Self {
            position_radii,
            point_cloud_bounds,
            picking_ids,
            colors,
        }
    }

    fn heap_size_bytes(&self) -> u64 {
        let Self {
            position_radii,
            point_cloud_bounds: _,
            picking_ids,
            colors,
        } = self;

        (position_radii.capacity() * std::mem::size_of::<PositionRadius>()) as u64
            + picking_ids.heap_size_bytes()
            + colors.heap_size_bytes()
    }
}

// --- GaussianSplats3DCache ---

struct GaussianSplats3DCacheEntry {
    cpu: Arc<GaussianSplats3DCpu>,
    last_used_generation: u64,
}

/// Caches [`GaussianSplats3DCpu`] to avoid recomputing colors, radii, etc. every frame.
#[derive(Default)]
pub struct GaussianSplats3DCache {
    cache: IntMap<Hash64, GaussianSplats3DCacheEntry>,
    generation: u64,
}

impl GaussianSplats3DCache {
    /// `key` must cover everything that affects the output of [`GaussianSplats3DCpu::compute`].
    fn entry(
        &mut self,
        key: Hash64,
        compute: impl FnOnce() -> GaussianSplats3DCpu,
    ) -> Arc<GaussianSplats3DCpu> {
        let entry = self
            .cache
            .entry(key)
            .or_insert_with(|| GaussianSplats3DCacheEntry {
                cpu: Arc::new(compute()),
                last_used_generation: 0,
            });
        entry.last_used_generation = self.generation;
        entry.cpu.clone()
    }
}

impl Cache for GaussianSplats3DCache {
    fn name(&self) -> &'static str {
        "GaussianSplats3DCache"
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

impl re_byte_size::SizeBytes for GaussianSplats3DCache {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            cache,
            generation: _,
        } = self;
        cache
            .values()
            .map(|entry| entry.cpu.heap_size_bytes() + std::mem::size_of_val(&entry.cpu) as u64)
            .sum::<u64>()
            + (cache.capacity() * std::mem::size_of::<(Hash64, GaussianSplats3DCacheEntry)>())
                as u64
    }
}

impl re_byte_size::MemUsageTreeCapture for GaussianSplats3DCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        re_byte_size::MemUsageTree::Bytes(self.total_size_bytes())
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl GaussianSplats3DVisualizer {
    fn process_data<'a>(
        view_data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        point_builder: &mut PointCloudBuilder<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: impl Iterator<Item = GaussianSplats3DComponentData<'a>>,
    ) {
        re_tracing::profile_function!();
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.centers.len();
            if num_instances == 0 {
                continue;
            }

            // The gaussian data is all `compute` looks at, and `query_result_hash` covers it.
            let cache_key = Hash64::hash((data.query_result_hash, data.index));

            let cpu = ctx.store_ctx().memoizer(|c: &mut GaussianSplats3DCache| {
                c.entry(cache_key, || GaussianSplats3DCpu::compute(ctx, &data))
            });

            for world_from_obj in ent_context
                .transform_info
                .target_from_instances()
                .iter()
                .map(|transform| transform.as_affine3a())
            {
                re_tracing::profile_scope!("one-transform");

                let point_batch = point_builder
                    .batch(entity_path.to_string())
                    // Gaussians are soft blobs, not lit spheres.
                    // Note that this also means the per-gaussian opacity (the alpha of the
                    // color) is ignored: every splat is drawn fully opaque, so scenes render
                    // denser than they should. A real splat renderer will fix that.
                    .enable_shading(false)
                    .world_from_obj(world_from_obj)
                    .outline_mask_ids(ent_context.highlight.overall)
                    .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

                let mut point_range_builder =
                    point_batch.add_points(&cpu.position_radii, &cpu.colors, &cpu.picking_ids);

                // Determine if there's any sub-ranges that need extra highlighting.
                #[expect(clippy::iter_over_hash_type)]
                // Non-overlapping per-instance mask ranges.
                for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                    if highlighted_key.get() < num_instances as u64 {
                        let highlighted_point_index = highlighted_key.get() as u32;
                        point_range_builder = point_range_builder
                            .push_additional_outline_mask_ids_for_range(
                                highlighted_point_index..highlighted_point_index + 1,
                                *instance_mask_ids,
                            );
                    }
                }

                view_data.add_bounding_box_and_region_of_interest(
                    entity_path.hash(),
                    cpu.point_cloud_bounds.bbox,
                    cpu.point_cloud_bounds.region_of_interest,
                    world_from_obj,
                    SpaceKind::ThreeD,
                );
            }
        }
    }
}

impl IdentifiedViewSystem for GaussianSplats3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "GaussianSplats3D"
        )
    }
}

impl VisualizerSystem for GaussianSplats3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Position3D>(
            &GaussianSplats3D::descriptor_centers(),
            &GaussianSplats3D::all_components(),
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

        let mut point_builder = PointCloudBuilder::new(ctx.viewer_ctx.render_ctx());
        point_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<GaussianSplats3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                re_tracing::profile_scope!("GaussianSplats3D");

                let all_centers =
                    results.iter_required(GaussianSplats3D::descriptor_centers().component);

                let num_centers: usize = {
                    re_tracing::profile_scope!("num_centers");
                    all_centers
                        .chunks()
                        .iter()
                        .flat_map(|chunk| chunk.iter_slices::<[f32; 3]>())
                        .map(|centers| centers.len())
                        .sum()
                };

                if num_centers == 0 {
                    return Ok(());
                }

                point_builder.reserve(num_centers)?;
                let all_scales =
                    results.iter_optional(GaussianSplats3D::descriptor_scales().component);
                let all_colors =
                    results.iter_optional(GaussianSplats3D::descriptor_colors().component);

                let query_result_hash = results.query_result_hash();

                let results_iter = re_query::range_zip_1x2(
                    all_centers.slice::<[f32; 3]>(),
                    all_scales.slice::<[f32; 3]>(),
                    all_colors.slice::<u32>(),
                )
                .map(|(index, centers, scales, colors)| {
                    GaussianSplats3DComponentData {
                        index,
                        query_result_hash,
                        centers: bytemuck::cast_slice(centers),
                        scales: scales.map_or(&[], |scales| bytemuck::cast_slice(scales)),
                        colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                    }
                });

                Self::process_data(
                    &mut view_data,
                    ctx,
                    &mut point_builder,
                    spatial_ctx,
                    results_iter,
                );

                Ok(())
            },
        )?;

        Ok(output
            .with_draw_data([point_builder.into_draw_data()?.into()])
            .with_visualizer_data(view_data))
    }
}
