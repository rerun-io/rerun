use std::sync::Arc;

use half::f16;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use parking_lot::Mutex;
use re_byte_size::SizeBytes as _;
use re_entity_db::EntityDb;
use re_log_types::hash::Hash64;
use re_renderer::{
    GaussianShCoefficient, GaussianSplatBuilder, PickingLayerInstanceId, Rgba32Unmul,
    SortOrderCache,
};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::GaussianSplats3D;
use re_sdk_types::components;
use re_sdk_types::components::{
    Color, Position3D, RotationQuat, Scale3D, SphericalHarmonics3Rgb, SphericalHarmonicsDegree,
};
use re_viewer_context::{
    Cache, IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use crate::SpaceKind;
use crate::contexts::SpatialSceneVisualizerInstructionContext;

// ---

/// The scale (standard deviation) used for gaussians that don't specify one, in scene units.
// TODO(RR-3840): create a proper default-provider for this
const FALLBACK_SCALE: f32 = 0.01;

/// Gaussians with a peak opacity (alpha, 0-255) below this are excluded from the bounding box.
///
/// 3DGS reconstructions often contain sparse, near-invisible "floater" gaussians far from the
/// object; including them would blow up the bounds far beyond what's actually visible.
const MIN_OPACITY_FOR_BOUNDS: u8 = 5; // ~0.02

/// Renders [`GaussianSplats3D`] via [`re_renderer`]'s gaussian splat renderer.
///
/// Each gaussian's 3D covariance is projected to a screen-space ellipse and alpha-blended
/// back-to-front, with view-dependent color from the spherical harmonics coefficients.
#[derive(Default)]
pub struct GaussianSplats3DVisualizer;

struct GaussianSplats3DComponentData<'a> {
    index: (re_log_types::TimeInt, re_chunk_store::RowId),
    query_result_hash: Hash64,

    // Point of views
    centers: &'a [Position3D],

    // Clamped to edge
    scales: &'a [Scale3D],
    quaternions: &'a [RotationQuat],
    colors: &'a [Color],
    sh_coefficients: &'a [SphericalHarmonics3Rgb],

    // Non-repeated
    spherical_harmonics_degree: Option<SphericalHarmonicsDegree>,
}

/// Processed/computed gaussian cloud data ready for rendering.
///
/// This bundles together the results of processing raw component data
/// (computing colors, bounding boxes, etc.)
/// so that it can be memoized based on `data.query_hash`.
#[derive(re_byte_size::SizeBytes)]
struct GaussianSplats3DCpu {
    centers: Vec<glam::Vec3>,
    scales: Vec<glam::Vec3>,
    rotations: Vec<glam::Quat>,
    colors: Vec<Rgba32Unmul>,

    /// Padded to RGBA so it can be uploaded straight into the `Rgba16Float` SH texture.
    sh_coefficients: Vec<[GaussianShCoefficient; 15]>,
    picking_ids: Vec<PickingLayerInstanceId>,
    point_cloud_bounds: re_renderer::util::PointCloudBounds,

    /// Scratch buffers holding the back-to-front gaussian ordering, reused across frames to speed
    /// up the per-frame CPU sort (see [`re_renderer::GaussianSplatBuilder`]).
    ///
    /// Each instance transform has its own cache, which tracks ordering per rendered view.
    ///
    /// Lives here so its lifetime and invalidation piggyback on the [`GaussianSplats3DCache`]
    /// memoization: when the underlying data changes, fresh (empty) caches are created
    /// automatically.
    sort_order_caches: Mutex<Vec<SortOrderCache>>,
}

impl GaussianSplats3DCpu {
    fn compute(ctx: &QueryContext<'_>, data: &GaussianSplats3DComponentData<'_>) -> Self {
        let num_instances = data.centers.len();
        re_tracing::profile_function!(re_format::format_uint(num_instances));

        let picking_ids = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "picking_ids");
            (0..num_instances)
                .map(|i| PickingLayerInstanceId(i as _))
                .collect_vec()
        };

        let centers: Vec<glam::Vec3> = bytemuck::cast_slice(data.centers).to_vec();

        let scales: Vec<glam::Vec3> = if data.scales.is_empty() {
            vec![glam::Vec3::splat(FALLBACK_SCALE); num_instances]
        } else {
            bytemuck::cast_slice(data.scales).to_vec()
        };

        let rotations: Vec<glam::Quat> = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "rotations");
            data.quaternions
                .iter()
                .map(|q| glam::Quat::try_from(*q).unwrap_or(glam::Quat::IDENTITY))
                .collect()
        };

        // Unmultiplied sRGB RGBA (NOT `Color32`, which is premultiplied: premultiplying in
        // gamma space badly darkens the accumulation of many low-opacity gaussians and
        // quantizes away the color of faint ones).
        let colors: Vec<Rgba32Unmul> = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "colors");
            let fallback: components::Color =
                typed_fallback_for(ctx, GaussianSplats3D::descriptor_colors().component);
            let last = data.colors.last().copied().unwrap_or(fallback);
            std::iter::chain(data.colors.iter().copied(), std::iter::repeat(last))
                .take(num_instances)
                .map(|c| Rgba32Unmul::from_rgba_unmul_array(c.to_array()))
                .collect()
        };

        // Widen each RGB coefficient to RGBA so the renderer can memcpy it into its texture.
        let sh_coefficients: Vec<[GaussianShCoefficient; 15]> = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "sh_coefficients");
            data.sh_coefficients
                .iter()
                .map(|sh| std::array::from_fn(|i| GaussianShCoefficient::from_rgb(sh.0.0[i])))
                .collect()
        };

        let point_cloud_bounds = {
            re_tracing::profile_scope_if!(100_000 < num_instances, "bounding_box");
            // Exclude near-invisible floater gaussians so they don't blow up the bounds.
            let opaque_centers: Vec<glam::Vec3> = std::iter::zip(&centers, &colors)
                .filter(|(_, color)| MIN_OPACITY_FOR_BOUNDS <= color.0[3])
                .map(|(center, _)| *center)
                .collect();
            if opaque_centers.is_empty() {
                re_renderer::util::point_cloud_bounds(&centers)
            } else {
                re_renderer::util::point_cloud_bounds(&opaque_centers)
            }
        };

        Self {
            centers,
            scales,
            rotations,
            colors,
            sh_coefficients,
            picking_ids,
            point_cloud_bounds,
            sort_order_caches: Mutex::new(Vec::new()),
        }
    }

    /// The back-to-front sort cache for the given instance transform, creating it on first use.
    fn sort_order_cache(&self, transform_index: usize) -> SortOrderCache {
        let mut caches = self.sort_order_caches.lock();
        caches.resize_with(transform_index + 1, SortOrderCache::default);
        caches[transform_index].clone()
    }
}

// --- GaussianSplats3DCache ---

struct GaussianSplats3DCacheEntry {
    cpu: Arc<GaussianSplats3DCpu>,
    last_used_generation: u64,
}

/// Caches [`GaussianSplats3DCpu`] to avoid recomputing colors etc. every frame.
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
        splat_builder: &mut GaussianSplatBuilder<'_>,
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

            // Gaussians don't use the annotation context, so the query results are the only
            // input to `compute`.
            let cache_key = Hash64::hash((data.query_result_hash, data.index));

            let cpu = ctx.store_ctx().memoizer(|c: &mut GaussianSplats3DCache| {
                c.entry(cache_key, || GaussianSplats3DCpu::compute(ctx, &data))
            });

            let sh_num_coefficients = data
                .spherical_harmonics_degree
                .unwrap_or_else(|| {
                    typed_fallback_for(
                        ctx,
                        GaussianSplats3D::descriptor_spherical_harmonics_degree().component,
                    )
                })
                .num_coefficients();
            // Degree 0 is the base color alone, so nothing needs uploading at all. Otherwise the
            // (degree-independent) cache is truncated on the way to the GPU.
            let sh_coefficients: &[[GaussianShCoefficient; 15]] = if sh_num_coefficients == 0 {
                &[]
            } else {
                &cpu.sh_coefficients
            };

            for (transform_index, world_from_obj) in ent_context
                .transform_info
                .target_from_instances()
                .iter()
                .map(|transform| transform.as_affine3a())
                .enumerate()
            {
                re_tracing::profile_scope!("one-transform");

                // Seed this frame's back-to-front sort from the previous frame's ordering. The
                // cache lives in `cpu`, so it persists across frames (per transform, per view)
                // and is reset automatically when the underlying data changes.
                let mut splat_batch = splat_builder
                    .batch(entity_path.to_string())
                    .world_from_obj(world_from_obj)
                    .object_space_bounding_box(cpu.point_cloud_bounds.bbox)
                    .outline_mask_ids(ent_context.highlight.overall)
                    .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()))
                    .sort_order(cpu.sort_order_cache(transform_index))
                    .add_gaussians(
                        &cpu.centers,
                        &cpu.scales,
                        &cpu.rotations,
                        &cpu.colors,
                        sh_coefficients,
                        sh_num_coefficients,
                        &cpu.picking_ids,
                    );

                // Determine if there's any sub-ranges that need extra highlighting.
                #[expect(clippy::iter_over_hash_type)]
                // Non-overlapping per-instance mask ranges.
                for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                    if highlighted_key.get() < num_instances as u64 {
                        let highlighted_index = highlighted_key.get() as u32;
                        splat_batch = splat_batch.push_additional_outline_mask_ids_for_range(
                            highlighted_index..highlighted_index + 1,
                            *instance_mask_ids,
                        );
                    }
                }
                drop(splat_batch);

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

        let mut splat_builder = GaussianSplatBuilder::new(ctx.viewer_ctx.render_ctx());

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
                if all_centers.is_empty() {
                    return Ok(());
                }

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

                splat_builder.reserve(num_centers)?;
                let all_scales =
                    results.iter_optional(GaussianSplats3D::descriptor_scales().component);
                let all_quaternions =
                    results.iter_optional(GaussianSplats3D::descriptor_quaternions().component);
                let all_colors =
                    results.iter_optional(GaussianSplats3D::descriptor_colors().component);
                let all_sh_coefficients =
                    results.iter_optional(GaussianSplats3D::descriptor_sh_coefficients().component);
                let all_spherical_harmonics_degree = results.iter_optional(
                    GaussianSplats3D::descriptor_spherical_harmonics_degree().component,
                );

                let query_result_hash = results.query_result_hash();

                let results_iter = re_query::range_zip_1x5(
                    all_centers.slice::<[f32; 3]>(),
                    all_scales.slice::<[f32; 3]>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_colors.slice::<u32>(),
                    all_sh_coefficients.slice::<[[f16; 3]; 15]>(),
                    all_spherical_harmonics_degree.slice::<u32>(),
                )
                .map(
                    |(
                        index,
                        centers,
                        scales,
                        quaternions,
                        colors,
                        sh_coefficients,
                        spherical_harmonics_degree,
                    )| {
                        GaussianSplats3DComponentData {
                            index,
                            query_result_hash,
                            centers: bytemuck::cast_slice(centers),
                            scales: scales.map_or(&[], |scales| bytemuck::cast_slice(scales)),
                            quaternions: quaternions
                                .map_or(&[], |quaternions| bytemuck::cast_slice(quaternions)),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            sh_coefficients: sh_coefficients
                                .map_or(&[], |sh| bytemuck::cast_slice(sh)),
                            spherical_harmonics_degree: spherical_harmonics_degree
                                .and_then(|d| d.first().copied())
                                .map(|d| SphericalHarmonicsDegree(d.into())),
                        }
                    },
                );

                Self::process_data(
                    &mut view_data,
                    ctx,
                    &mut splat_builder,
                    spatial_ctx,
                    results_iter,
                );

                Ok(())
            },
        )?;

        Ok(output
            .with_draw_data([splat_builder.into_draw_data()?.into()])
            .with_visualizer_data(view_data))
    }
}
