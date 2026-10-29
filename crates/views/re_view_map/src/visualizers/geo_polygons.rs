use std::collections::HashMap;
use std::sync::Arc;

use re_log_types::{EntityPath, Instance, hash::Hash64};
use re_renderer::mesh::{CpuMesh, GpuMesh, Material};
use re_renderer::renderer::{GpuMeshInstance, MeshDrawData};
use re_sdk_types::Archetype as _;
use re_sdk_types::archetypes::GeoPolygons;
use re_sdk_types::components::{Color, GeoPolygon};
use re_view::{DataResultQuery as _, VisualizerInstructionQueryResults};
use re_viewer_context::{
    Cache, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewHighlights, ViewQuery,
    ViewSystemExecutionError, ViewerReportSeverity, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::polygon_geometry::{PolygonGeometry, triangulate};

type PreparedPolygon = Result<Arc<PolygonGeometry>, &'static str>;

struct CacheEntry {
    geometry: PreparedPolygon,
    last_used_generation: u64,
}

/// Geometry preparation is independent of screen projection and highlights.
/// Resolved query identity also covers chunk replacement and component mapping changes.
#[derive(Default)]
struct PolygonCache {
    entries: HashMap<Hash64, CacheEntry>,
    generation: u64,
}

impl PolygonCache {
    fn entry(&mut self, key: Hash64, positions: &[[f64; 2]], lengths: &[u64]) -> PreparedPolygon {
        let entry = self.entries.entry(key).or_insert_with(|| CacheEntry {
            geometry: triangulate(positions, lengths).map(Arc::new),
            last_used_generation: self.generation,
        });
        entry.last_used_generation = self.generation;
        entry.geometry.clone()
    }
}

impl Cache for PolygonCache {
    fn name(&self) -> &'static str {
        "GeoPolygonsCache"
    }

    fn begin_frame(&mut self) {
        self.entries
            .retain(|_, entry| entry.last_used_generation == self.generation);
        self.generation += 1;
    }

    fn purge_memory(&mut self) {
        self.entries.clear();
    }
}

impl re_byte_size::MemUsageTreeCapture for PolygonCache {
    fn capture_mem_usage_tree(&self) -> re_byte_size::MemUsageTree {
        let geometry_bytes = self
            .entries
            .values()
            .map(|entry| {
                entry.geometry.as_ref().map_or(0, |geometry| {
                    geometry.lat_lon.capacity() * std::mem::size_of::<[f64; 2]>()
                        + geometry.triangles.capacity() * std::mem::size_of::<[u32; 3]>()
                        + std::mem::size_of::<PolygonGeometry>()
                })
            })
            .sum::<usize>();
        re_byte_size::MemUsageTree::Bytes(
            (geometry_bytes + self.entries.capacity() * std::mem::size_of::<(Hash64, CacheEntry)>())
                as u64,
        )
    }
}

#[derive(Clone)]
struct PolygonInstance {
    cache_key: Hash64,
    geometry: Arc<PolygonGeometry>,
    color: re_renderer::Color32,
    instance: Instance,
}

#[derive(Default, Clone)]
pub struct GeoPolygonsOutput {
    batches: Vec<(EntityPath, Vec<PolygonInstance>)>,
}

/// Per-view projected meshes. Only meshes used in the previous frame are retained.
#[derive(Default)]
pub struct GeoPolygonsDrawCache {
    meshes: HashMap<Hash64, Arc<GpuMesh>>,
    #[cfg(test)]
    uploads: usize,
}

impl GeoPolygonsDrawCache {
    pub fn heap_size_bytes(&self) -> u64 {
        // GPU allocations are already accounted for by the renderer's resource pools.
        (self.meshes.capacity() * std::mem::size_of::<(Hash64, Arc<GpuMesh>)>()
            + self.meshes.len() * std::mem::size_of::<GpuMesh>()) as u64
    }

    #[cfg(test)]
    pub fn upload_count(&self) -> usize {
        self.uploads
    }

    #[cfg(test)]
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
}

fn projection_key(projector: &walkers::Projector) -> Hash64 {
    // Two inverse-projected pixels identify this axis-aligned Mercator transform in f64.
    // Projecting distant geographic anchors to f32 would lose precision at high zoom levels.
    Hash64::hash([egui::Vec2::ZERO, egui::Vec2::splat(1.0)].map(|pixel| {
        let position = projector.unproject(pixel);
        [position.x().to_bits(), position.y().to_bits()]
    }))
}

#[derive(Default)]
pub struct GeoPolygonsVisualizer;

impl IdentifiedViewSystem for GeoPolygonsVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "GeoPolygons"
        )
    }
}

impl VisualizerSystem for GeoPolygonsVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<GeoPolygon>(
            &GeoPolygons::descriptor_polygons(),
            &GeoPolygons::all_components(),
        )
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        _context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let output = VisualizerExecutionOutput::default();
        let mut batches = Vec::new();
        for (data_result, instruction) in
            view_query.iter_visualizer_instruction_for(Self::identifier())
        {
            let queried = data_result.query_archetype_with_history::<GeoPolygons>(
                ctx,
                view_query,
                instruction,
                None,
            );
            let results = VisualizerInstructionQueryResults::new(instruction, &queried, &output);
            let polygons_component = GeoPolygons::descriptor_polygons().component;
            let all_polygons = results.iter_required(polygons_component);
            let all_colors = results.iter_optional(GeoPolygons::descriptor_colors().component);
            let source_hash = results.component_source_hash(polygons_component);
            let query_hash = results.query_result_hash();
            let query_context =
                ctx.query_context(data_result, view_query.latest_at_query(), instruction.id);
            let fallback: Color =
                typed_fallback_for(&query_context, GeoPolygons::descriptor_colors().component);
            let mut batch = Vec::new();
            // The two fields share the same required row identity. Both paths are zero-copy.
            for (index, positions, lengths, colors) in re_query::range_zip_2x1(
                all_polygons.slice_from_struct_field::<&[[f64; 2]]>("lat_lon"),
                all_polygons.slice_from_struct_field::<&[u64]>("ring_lengths"),
                all_colors.slice::<u32>(),
            ) {
                let colors = colors.unwrap_or_default();
                let last_color = colors.last().copied().unwrap_or(fallback.0.0);
                for (instance_index, (positions, lengths)) in
                    itertools::izip!(positions.as_slice(), lengths.as_slice()).enumerate()
                {
                    let key = Hash64::hash((
                        data_result.entity_path.hash(),
                        source_hash,
                        query_hash,
                        index.1,
                        instance_index,
                    ));
                    let geometry = query_context
                        .store_ctx()
                        .memoizer(|cache: &mut PolygonCache| cache.entry(key, positions, lengths));
                    match geometry {
                        Ok(geometry) => batch.push(PolygonInstance {
                            cache_key: key,
                            geometry,
                            color: Color::new(
                                colors.get(instance_index).copied().unwrap_or(last_color),
                            )
                            .into(),
                            instance: Instance::from(instance_index as u64),
                        }),
                        Err(error) => results.report_for_component(
                            polygons_component,
                            ViewerReportSeverity::Error,
                            format!("Polygon {instance_index}: {error}"),
                        ),
                    }
                }
            }
            batches.push((data_result.entity_path.clone(), batch));
        }
        Ok(output.with_visualizer_data(GeoPolygonsOutput { batches }))
    }
}

impl GeoPolygonsOutput {
    pub fn span(&self) -> Option<super::GeoSpan> {
        super::GeoSpan::from_lat_long(
            self.batches
                .iter()
                .flat_map(|(_, batch)| batch)
                .flat_map(|instance| &instance.geometry.lat_lon)
                .map(|position| (position[0], position[1])),
        )
    }

    pub fn queue_draw_data(
        &self,
        render_ctx: &re_renderer::RenderContext,
        view_builder: &mut re_renderer::ViewBuilder,
        projector: &walkers::Projector,
        highlights: &ViewHighlights,
        cache: &mut GeoPolygonsDrawCache,
    ) -> Result<(), ViewSystemExecutionError> {
        let mut previous_meshes = std::mem::take(&mut cache.meshes);
        let projection_key = projection_key(projector);
        let mut instances = Vec::new();
        let polygon_count = self
            .batches
            .iter()
            .map(|(_, batch)| batch.len())
            .sum::<usize>();
        for (entity_path, batch) in &self.batches {
            for polygon in batch {
                // Stable depth gives transparent overlaps a deterministic order and keeps fills behind points/lines.
                let depth = 0.2 - 0.1 * (instances.len() + 1) as f32 / (polygon_count + 1) as f32;
                let key = Hash64::hash((
                    polygon.cache_key,
                    projection_key,
                    polygon.color,
                    depth.to_bits(),
                ));
                let gpu_mesh = if let Some(mesh) = previous_meshes.remove(&key) {
                    mesh
                } else {
                    let positions: Vec<glam::Vec3> = polygon
                        .geometry
                        .lat_lon
                        .iter()
                        .map(|&[lat, lon]| {
                            let position = projector.project(walkers::lat_lon(lat, lon));
                            glam::vec3(position.x, position.y, depth)
                        })
                        .collect();
                    let num_vertices = positions.len();
                    let label: re_renderer::Label = entity_path.to_string().into();
                    let material = Material {
                        label: label.clone(),
                        index_range: re_span::Span::from_start_len(
                            0,
                            polygon.geometry.triangles.len() as u32 * 3,
                        ),
                        albedo: render_ctx
                            .texture_manager_2d
                            .white_texture_unorm_handle()
                            .clone(),
                        albedo_factor: re_renderer::Rgba::from(polygon.color),
                    };
                    let mesh = CpuMesh {
                        label,
                        triangle_indices: polygon
                            .geometry
                            .triangles
                            .iter()
                            .copied()
                            .map(glam::UVec3::from)
                            .collect(),
                        bbox: re_renderer::util::bounding_box_from_points(
                            positions.iter().copied(),
                        ),
                        vertex_positions: positions,
                        vertex_colors: vec![re_renderer::Rgba32Unmul::WHITE; num_vertices],
                        vertex_normals: vec![glam::Vec3::ZERO; num_vertices], // Unlit map fill.
                        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],
                        materials: vec![material].into(),
                    };
                    let gpu_mesh = Arc::new(GpuMesh::new(render_ctx, &mesh).map_err(|error| {
                        ViewSystemExecutionError::DrawDataCreationError(Arc::new(error))
                    })?);
                    #[cfg(test)]
                    {
                        cache.uploads += 1;
                    }
                    gpu_mesh
                };
                cache.meshes.insert(key, gpu_mesh.clone());
                let mut instance = GpuMeshInstance::new(gpu_mesh);
                instance.outline_mask_ids = highlights
                    .entity_outline_mask(entity_path.hash())
                    .index_outline_mask(polygon.instance);
                instance.picking_layer_id = re_view::picking_layer_id_from_instance_path_hash(
                    re_entity_db::InstancePathHash::instance(entity_path, polygon.instance),
                );
                instances.push(instance);
            }
        }
        let draw_data = MeshDrawData::new(render_ctx, &instances)
            .map_err(|error| ViewSystemExecutionError::DrawDataCreationError(Arc::new(error)))?
            .with_sort_axis(glam::Vec3A::Z);
        view_builder.queue_draw(render_ctx, draw_data)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangulation_is_shared_and_old_rows_expire() {
        let mut cache = PolygonCache::default();
        let positions = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]];
        let key = Hash64::hash(1);
        let first = cache.entry(key, &positions, &[3]).unwrap();
        cache.begin_frame();
        let second = cache.entry(key, &positions, &[3]).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        cache.begin_frame();
        cache.begin_frame();
        assert!(cache.entries.is_empty());
        let third = cache.entry(Hash64::hash(2), &positions, &[3]).unwrap();
        assert!(!Arc::ptr_eq(&first, &third));
        cache.purge_memory();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn projected_mesh_identity_tracks_pan_zoom_and_viewport() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 240.0));
        let mut memory = walkers::MapMemory::default();
        memory.set_zoom(12.0).unwrap();
        let center = walkers::lat_lon(48.85, 2.3);
        let key = projection_key(&walkers::Projector::new(rect, &memory, center));
        assert_eq!(
            key,
            projection_key(&walkers::Projector::new(rect, &memory, center))
        );
        assert_ne!(
            key,
            projection_key(&walkers::Projector::new(
                rect,
                &memory,
                walkers::lat_lon(48.850_001, 2.3),
            ))
        );
        assert_ne!(
            key,
            projection_key(&walkers::Projector::new(
                rect.translate(egui::vec2(1.0, 0.0)),
                &memory,
                center,
            ))
        );
        memory.set_zoom(13.0).unwrap();
        assert_ne!(
            key,
            projection_key(&walkers::Projector::new(rect, &memory, center))
        );
    }
}
