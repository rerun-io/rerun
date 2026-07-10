use std::iter;
use std::sync::Arc;

use re_chunk_store::RowId;
use re_log_types::Instance;
use re_log_types::hash::Hash64;
use re_renderer::renderer::GpuMeshInstance;
use re_sdk_types::archetypes::Planes3D;
use re_sdk_types::components::{
    ClassId, Color, FillMode, HalfSize2D, HalfSize3D, ImageFormat, Plane3D, Radius, ShowLabels,
};
use re_sdk_types::datatypes::{Blob, Rgba32};
use re_sdk_types::reflection::Enum as _;
use re_sdk_types::{Archetype as _, ArrowString, components};
use re_view::clamped_or_else;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use super::utilities::{ProcMeshBatch, ProcMeshDrawableBuilder};
use crate::caches::{AnyMesh, MeshCache, MeshCacheKey};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::mesh_loader::NativeMesh3D;
use crate::proc_mesh;

// ---
#[derive(Default)]
pub struct Planes3DVisualizer;

impl Planes3DVisualizer {
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        batches: impl Iterator<Item = Planes3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        for batch in batches {
            let num_instances = batch.planes.len().max(batch.half_sizes.len());
            let planes = clamped_or_else(batch.planes, || Plane3D::XY).take(num_instances);
            let half_sizes =
                clamped_or_else(batch.half_sizes, HalfSize2D::default).take(num_instances);

            let mut centers = Vec::with_capacity(num_instances);
            let mut quaternions = Vec::with_capacity(num_instances);
            let mut half_sizes3d = Vec::with_capacity(num_instances);

            for (plane, half_size) in std::iter::zip(planes, half_sizes) {
                let (center, quaternion) = plane_center_and_rotation(plane);
                centers.push(components::Translation3D::from(center));
                quaternions.push(components::RotationQuat::from(quaternion));
                half_sizes3d.push(HalfSize3D::new(half_size.x(), half_size.y(), 0.0));
            }

            let has_texture =
                batch.albedo_texture_buffer.is_some() && batch.albedo_texture_format.is_some();

            if has_texture && batch.fill_mode.has_solid() {
                add_textured_plane_mesh(
                    builder,
                    query_context,
                    ent_context,
                    &batch,
                    &centers,
                    &quaternions,
                    &half_sizes3d,
                )?;
            }

            if !has_texture || batch.fill_mode.has_wireframe() {
                let fill_mode = if has_texture {
                    FillMode::MajorWireframe
                } else {
                    batch.fill_mode
                };

                builder.add_batch(
                    query_context,
                    ent_context,
                    Planes3D::descriptor_colors().component,
                    Planes3D::descriptor_line_radii().component,
                    Planes3D::descriptor_show_labels().component,
                    glam::Affine3A::IDENTITY,
                    ProcMeshBatch {
                        half_sizes: &half_sizes3d,
                        centers: &centers,
                        rotation_axis_angles: &[],
                        quaternions: &quaternions,
                        meshes: iter::repeat(proc_mesh::ProcMeshKey::Plane),
                        fill_modes: iter::repeat(fill_mode),
                        line_radii: batch.line_radii,
                        colors: batch.colors,
                        labels: &batch.labels,
                        show_labels: batch.show_labels,
                        class_ids: batch.class_ids,
                    },
                )?;
            }
        }

        Ok(())
    }
}

// ---

struct Planes3DComponentData<'a> {
    index: (re_log_types::TimeInt, RowId),
    query_result_hash: Hash64,
    planes: &'a [Plane3D],
    half_sizes: &'a [HalfSize2D],
    colors: &'a [Color],
    labels: Vec<ArrowString>,
    line_radii: &'a [Radius],
    class_ids: &'a [ClassId],
    show_labels: Option<ShowLabels>,
    fill_mode: FillMode,
    albedo_factor: Option<Rgba32>,
    albedo_texture_buffer: Option<Blob>,
    albedo_texture_format: Option<re_sdk_types::datatypes::ImageFormat>,
}

fn add_textured_plane_mesh(
    builder: &mut ProcMeshDrawableBuilder<'_>,
    query_context: &QueryContext<'_>,
    ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
    batch: &Planes3DComponentData<'_>,
    centers: &[components::Translation3D],
    quaternions: &[components::RotationQuat],
    half_sizes: &[HalfSize3D],
) -> Result<(), ViewSystemExecutionError> {
    let entity_path = query_context.target_entity_path;

    let mut vertex_positions = Vec::with_capacity(half_sizes.len() * 4);
    let mut vertex_normals = Vec::with_capacity(half_sizes.len() * 4);
    let mut vertex_texcoords = Vec::with_capacity(half_sizes.len() * 4);
    let mut triangle_indices = Vec::with_capacity(half_sizes.len() * 2);
    let mut vertex_colors = Vec::with_capacity(half_sizes.len() * 4);
    let mut obj_space_bounding_box = macaw::BoundingBox::nothing();

    for (i, (half_size, center, quaternion)) in
        itertools::izip!(half_sizes, centers, quaternions).enumerate()
    {
        let base = vertex_positions.len() as u32;
        let hx = half_size.x();
        let hy = half_size.y();
        let center = glam::Vec3::from(center.0);
        let rotation = glam::Quat::from_array(quaternion.0.0);
        let normal = rotation * glam::Vec3::Z;
        let corners = [
            glam::vec3(-hx, -hy, 0.0),
            glam::vec3(hx, -hy, 0.0),
            glam::vec3(hx, hy, 0.0),
            glam::vec3(-hx, hy, 0.0),
        ];

        for corner in corners {
            let position = center + rotation * corner;
            obj_space_bounding_box.extend(position);
            vertex_positions.push(position);
            vertex_normals.push(normal);
        }

        vertex_texcoords.extend([
            glam::vec2(0.0, 0.0),
            glam::vec2(1.0, 0.0),
            glam::vec2(1.0, 1.0),
            glam::vec2(0.0, 1.0),
        ]);
        triangle_indices.extend([
            glam::uvec3(base, base + 1, base + 2),
            glam::uvec3(base, base + 2, base + 3),
        ]);

        let color = batch
            .colors
            .get(i)
            .or_else(|| batch.colors.last())
            .map(|color| {
                let [r, g, b, a] = color.to_array();
                Rgba32::from_unmultiplied_rgba(r, g, b, a)
            })
            .unwrap_or(Rgba32::WHITE);
        vertex_colors.extend([color; 4]);
    }

    if vertex_positions.is_empty() {
        return Ok(());
    }

    let world_from_obj = ent_context
        .transform_info
        .single_transform_required_for_entity(entity_path, Planes3D::name())
        .as_affine3a();

    let transparent_fill_albedo = (batch.fill_mode == FillMode::TransparentFillMajorWireframe)
        .then(|| Rgba32::from_unmultiplied_rgba(255, 255, 255, 64));

    let picking_instance_hash = re_entity_db::InstancePathHash::entity_all(entity_path);
    let mesh = query_context.store_ctx().memoizer(|c: &mut MeshCache| {
        let key = MeshCacheKey {
            versioned_instance_path_hash: picking_instance_hash.versioned(batch.index.1),
            query_result_hash: batch.query_result_hash,
            media_type: None,
        };

        c.entry(
            entity_path,
            key.clone(),
            AnyMesh::Mesh {
                mesh: NativeMesh3D {
                    vertex_positions: &vertex_positions,
                    vertex_normals: Some(&vertex_normals),
                    vertex_colors: (!batch.colors.is_empty()).then_some(vertex_colors.as_slice()),
                    vertex_texcoords: Some(&vertex_texcoords),
                    triangle_indices: Some(&triangle_indices),
                    albedo_factor: batch.albedo_factor.or(transparent_fill_albedo),
                    albedo_texture_buffer: batch.albedo_texture_buffer.clone(),
                    albedo_texture_format: batch.albedo_texture_format,
                },
                texture_key: Hash64::hash(&key).hash64(),
            },
            builder.render_ctx,
        )
    });

    let Some(mesh) = mesh else {
        return Err(ViewSystemExecutionError::DrawDataCreationError(Arc::new(
            std::io::Error::other("Failed to allocate plane mesh"),
        )));
    };

    builder
        .solid_instances
        .extend(
            mesh.mesh_instances
                .iter()
                .map(|mesh_instance| GpuMeshInstance {
                    gpu_mesh: mesh_instance.gpu_mesh.clone(),
                    world_from_mesh: world_from_obj * mesh_instance.world_from_mesh,
                    outline_mask_ids: ent_context.highlight.index_outline_mask(Instance::ALL),
                    picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                        picking_instance_hash,
                    ),
                    additive_tint: re_renderer::Color32::BLACK,
                    cull_mode: None,
                }),
        );

    builder
        .data
        .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

    Ok(())
}

impl IdentifiedViewSystem for Planes3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Planes3D"
        )
    }
}

impl VisualizerSystem for Planes3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Plane3D>(
            &Planes3D::descriptor_planes(),
            &Planes3D::all_components(),
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
        let output = VisualizerExecutionOutput::default();
        let mut data = SpatialViewVisualizerData::default();
        let mut builder = ProcMeshDrawableBuilder::new(
            &mut data,
            ctx.viewer_ctx.render_ctx(),
            view_query,
            &output,
            "planes3d",
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Planes3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_planes = results.iter_required(Planes3D::descriptor_planes().component);
                let all_half_sizes =
                    results.iter_optional(Planes3D::descriptor_half_sizes().component);
                let all_colors = results.iter_optional(Planes3D::descriptor_colors().component);
                let all_labels = results.iter_optional(Planes3D::descriptor_labels().component);
                let all_fill_modes =
                    results.iter_optional(Planes3D::descriptor_fill_mode().component);
                let all_albedo_factors =
                    results.iter_optional(Planes3D::descriptor_albedo_factor().component);
                let all_albedo_buffers =
                    results.iter_optional(Planes3D::descriptor_albedo_texture_buffer().component);
                let all_albedo_formats =
                    results.iter_optional(Planes3D::descriptor_albedo_texture_format().component);
                let all_line_radii =
                    results.iter_optional(Planes3D::descriptor_line_radii().component);
                let all_show_labels =
                    results.iter_optional(Planes3D::descriptor_show_labels().component);
                let all_class_ids =
                    results.iter_optional(Planes3D::descriptor_class_ids().component);

                let query_result_hash = results.query_result_hash();

                let data = re_query::range_zip_1x10(
                    all_planes.slice::<[f32; 4]>(),
                    all_half_sizes.slice::<[f32; 2]>(),
                    all_colors.slice::<u32>(),
                    all_line_radii.slice::<f32>(),
                    all_fill_modes.slice::<u8>(),
                    all_albedo_factors.slice::<u32>(),
                    all_albedo_buffers.slice::<&[u8]>(),
                    all_albedo_formats.component_slow::<ImageFormat>(),
                    all_labels.slice::<String>(),
                    all_show_labels.slice::<bool>(),
                    all_class_ids.slice::<u16>(),
                )
                .map(
                    |(
                        _index,
                        planes,
                        half_sizes,
                        colors,
                        line_radii,
                        fill_modes,
                        albedo_factors,
                        albedo_buffers,
                        albedo_formats,
                        labels,
                        show_labels,
                        class_ids,
                    )| Planes3DComponentData {
                        index: _index,
                        query_result_hash,
                        planes: bytemuck::cast_slice(planes),
                        half_sizes: half_sizes.map_or(&[], bytemuck::cast_slice),
                        colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                        labels: labels.unwrap_or_default(),
                        line_radii: line_radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                        fill_mode: fill_modes
                            .and_then(|s| FillMode::from_integer_slice(s).next()?)
                            .unwrap_or_default(),
                        class_ids: class_ids
                            .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                        show_labels: show_labels
                            .map(|b| !b.is_empty() && b.value(0))
                            .map(Into::into),
                        albedo_factor: albedo_factors
                            .map(bytemuck::cast_slice)
                            .and_then(|albedo_factors: &[Rgba32]| albedo_factors.first().copied()),
                        albedo_texture_buffer: albedo_buffers
                            .unwrap_or_default()
                            .first()
                            .cloned()
                            .map(Into::into),
                        albedo_texture_format: albedo_formats
                            .unwrap_or_default()
                            .first()
                            .map(|format| format.0),
                    },
                );

                Self::process_data(&mut builder, ctx, spatial_ctx, data)?;

                Ok(())
            },
        )?;

        let draw_data = builder.into_draw_data()?;
        Ok(output.with_draw_data(draw_data).with_visualizer_data(data))
    }
}

fn plane_center_and_rotation(plane: Plane3D) -> (glam::Vec3, glam::Quat) {
    let raw = plane.0.0;
    let normal = glam::Vec3::new(raw[0], raw[1], raw[2]);
    let length = normal.length();
    let (normal, distance) = if normal.is_finite() && length > 0.0 {
        (normal / length, raw[3] / length)
    } else {
        (glam::Vec3::Z, 0.0)
    };

    (
        normal * distance,
        glam::Quat::from_rotation_arc(glam::Vec3::Z, normal),
    )
}
