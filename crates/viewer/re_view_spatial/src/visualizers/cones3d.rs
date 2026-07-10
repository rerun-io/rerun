use std::collections::HashMap;
use std::iter;
use std::sync::Arc;

use re_chunk_store::RowId;
use re_chunk_store::external::re_chunk::ChunkComponentIterItem;
use re_log_types::Instance;
use re_log_types::hash::Hash64;
use re_renderer::renderer::GpuMeshInstance;
use re_sdk_types::archetypes::Cones3D;
use re_sdk_types::components::{
    ClassId, Color, FillMode, HalfSize3D, ImageFormat, Length, Radius, ShowLabels,
};
use re_sdk_types::datatypes::{Blob, Rgba32};
use re_sdk_types::reflection::Enum as _;
use re_sdk_types::{Archetype as _, ArrowString, components};
use re_view::clamped_or_else;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::utilities::{ProcMeshBatch, ProcMeshDrawableBuilder};
use crate::caches::{AnyMesh, MeshCache, MeshCacheKey};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::mesh_loader::NativeMesh3D;
use crate::proc_mesh;

// ---
#[derive(Default)]
pub struct Cones3DVisualizer;

impl Cones3DVisualizer {
    fn process_data<'a>(
        builder: &mut ProcMeshDrawableBuilder<'_>,
        query_context: &QueryContext<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        batches: impl Iterator<Item = Cones3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        for batch in batches {
            let num_instances = batch.radii.len().max(batch.lengths.len());
            let lengths_iter = clamped_or_else(batch.lengths, || {
                typed_fallback_for::<Length>(query_context, Cones3D::descriptor_lengths().component)
            })
            .take(num_instances);
            let radii_iter = clamped_or_else(batch.radii, || {
                typed_fallback_for::<Radius>(query_context, Cones3D::descriptor_radii().component)
            })
            .take(num_instances);

            let half_sizes: Vec<HalfSize3D> = std::iter::zip(lengths_iter, radii_iter)
                .map(|(Length(length), Radius(radius))| {
                    let radius = clean_length(radius.0);
                    HalfSize3D::new(radius, radius, length.0 / 2.0)
                })
                .collect();

            let subdivisions = match batch.fill_mode {
                FillMode::DenseWireframe => 3,
                FillMode::Solid | FillMode::TransparentFillMajorWireframe => 4,
                FillMode::MajorWireframe => 10,
            };

            let proc_mesh_key = proc_mesh::ProcMeshKey::Cone {
                subdivisions,
                axes_only: batch.fill_mode.axes_only(),
            };

            let has_texture =
                batch.albedo_texture_buffer.is_some() && batch.albedo_texture_format.is_some();

            if has_texture && batch.fill_mode.has_solid() {
                add_textured_cone_mesh(builder, query_context, ent_context, &batch, &half_sizes)?;
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
                    Cones3D::descriptor_colors().component,
                    Cones3D::descriptor_line_radii().component,
                    Cones3D::descriptor_show_labels().component,
                    glam::Affine3A::IDENTITY,
                    ProcMeshBatch {
                        half_sizes: &half_sizes,
                        centers: batch.centers,
                        rotation_axis_angles: batch.rotation_axis_angles.as_slice(),
                        quaternions: batch.quaternions,
                        meshes: iter::repeat(proc_mesh_key),
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

struct Cones3DComponentData<'a> {
    index: (re_log_types::TimeInt, RowId),
    query_result_hash: Hash64,
    lengths: &'a [Length],
    radii: &'a [Radius],
    centers: &'a [components::Translation3D],
    rotation_axis_angles: ChunkComponentIterItem<components::RotationAxisAngle>,
    quaternions: &'a [components::RotationQuat],
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

struct ConeTextureData {
    albedo_factor: Option<Rgba32>,
    albedo_texture_buffer: Option<Blob>,
    albedo_texture_format: Option<re_sdk_types::datatypes::ImageFormat>,
}

fn add_textured_cone_mesh(
    builder: &mut ProcMeshDrawableBuilder<'_>,
    query_context: &QueryContext<'_>,
    ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
    batch: &Cones3DComponentData<'_>,
    half_sizes: &[HalfSize3D],
) -> Result<(), ViewSystemExecutionError> {
    let entity_path = query_context.target_entity_path;
    let subdivisions = 64;

    let mut vertex_positions = Vec::new();
    let mut vertex_normals = Vec::new();
    let mut vertex_texcoords = Vec::new();
    let mut triangle_indices = Vec::new();
    let mut vertex_colors = Vec::new();
    let mut obj_space_bounding_box = macaw::BoundingBox::nothing();

    let centers = clamped_component(batch.centers, half_sizes.len());
    let rotation_axis_angles =
        clamped_component(batch.rotation_axis_angles.as_slice(), half_sizes.len());
    let quaternions = clamped_component(batch.quaternions, half_sizes.len());

    for (instance_index, half_size) in half_sizes.iter().enumerate() {
        let radius = half_size.x().max(0.0);
        let half_length = half_size.z().max(0.0);
        if radius <= 0.0 || half_length <= 0.0 {
            continue;
        }

        let center = centers
            .get(instance_index)
            .map_or(glam::Vec3::ZERO, |center| glam::Vec3::from(center.0));
        let rotation = cone_rotation(
            rotation_axis_angles.get(instance_index),
            quaternions.get(instance_index),
        );

        let color = batch
            .colors
            .get(instance_index)
            .or_else(|| batch.colors.last())
            .map(|color| {
                let [r, g, b, a] = color.to_array();
                Rgba32::from_unmultiplied_rgba(r, g, b, a)
            });

        add_cone_instance_geometry(
            radius,
            half_length,
            center,
            rotation,
            color,
            subdivisions,
            &mut vertex_positions,
            &mut vertex_normals,
            &mut vertex_texcoords,
            &mut triangle_indices,
            &mut vertex_colors,
            &mut obj_space_bounding_box,
        );
    }

    if vertex_positions.is_empty() {
        return Ok(());
    }

    let world_from_obj = ent_context
        .transform_info
        .single_transform_required_for_entity(entity_path, Cones3D::name())
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
            std::io::Error::other("Failed to allocate cone mesh"),
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

fn add_cone_instance_geometry(
    radius: f32,
    half_length: f32,
    center: glam::Vec3,
    rotation: glam::Quat,
    color: Option<Rgba32>,
    subdivisions: u32,
    vertex_positions: &mut Vec<glam::Vec3>,
    vertex_normals: &mut Vec<glam::Vec3>,
    vertex_texcoords: &mut Vec<glam::Vec2>,
    triangle_indices: &mut Vec<glam::UVec3>,
    vertex_colors: &mut Vec<Rgba32>,
    obj_space_bounding_box: &mut macaw::BoundingBox,
) {
    let base_z = -half_length;
    let tip_z = half_length;
    let slope_normal_z = radius / (2.0 * half_length);

    for segment in 0..subdivisions {
        let u0 = segment as f32 / subdivisions as f32;
        let u1 = (segment + 1) as f32 / subdivisions as f32;
        let a0 = u0 * std::f32::consts::TAU;
        let a1 = u1 * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();

        let side_vertices = [
            (
                glam::vec3(radius * c0, radius * s0, base_z),
                glam::vec3(c0, s0, slope_normal_z).normalize(),
                glam::vec2(u0, 0.0),
            ),
            (
                glam::vec3(radius * c1, radius * s1, base_z),
                glam::vec3(c1, s1, slope_normal_z).normalize(),
                glam::vec2(u1, 0.0),
            ),
            (
                glam::vec3(0.0, 0.0, tip_z),
                glam::vec3(c0 + c1, s0 + s1, slope_normal_z).normalize(),
                glam::vec2((u0 + u1) * 0.5, 1.0),
            ),
        ];
        push_triangle(
            side_vertices,
            rotation,
            center,
            color,
            vertex_positions,
            vertex_normals,
            vertex_texcoords,
            triangle_indices,
            vertex_colors,
            obj_space_bounding_box,
        );

        let base_vertices = [
            (
                glam::vec3(0.0, 0.0, base_z),
                -glam::Vec3::Z,
                glam::vec2(0.5, 0.5),
            ),
            (
                glam::vec3(radius * c1, radius * s1, base_z),
                -glam::Vec3::Z,
                glam::vec2(0.5 + 0.5 * c1, 0.5 + 0.5 * s1),
            ),
            (
                glam::vec3(radius * c0, radius * s0, base_z),
                -glam::Vec3::Z,
                glam::vec2(0.5 + 0.5 * c0, 0.5 + 0.5 * s0),
            ),
        ];
        push_triangle(
            base_vertices,
            rotation,
            center,
            color,
            vertex_positions,
            vertex_normals,
            vertex_texcoords,
            triangle_indices,
            vertex_colors,
            obj_space_bounding_box,
        );
    }
}

fn push_triangle(
    vertices: [(glam::Vec3, glam::Vec3, glam::Vec2); 3],
    rotation: glam::Quat,
    center: glam::Vec3,
    color: Option<Rgba32>,
    vertex_positions: &mut Vec<glam::Vec3>,
    vertex_normals: &mut Vec<glam::Vec3>,
    vertex_texcoords: &mut Vec<glam::Vec2>,
    triangle_indices: &mut Vec<glam::UVec3>,
    vertex_colors: &mut Vec<Rgba32>,
    obj_space_bounding_box: &mut macaw::BoundingBox,
) {
    let base = vertex_positions.len() as u32;
    for (position, normal, texcoord) in vertices {
        let position = center + rotation * position;
        obj_space_bounding_box.extend(position);
        vertex_positions.push(position);
        vertex_normals.push(rotation * normal);
        vertex_texcoords.push(texcoord);
        if let Some(color) = color {
            vertex_colors.push(color);
        }
    }
    triangle_indices.push(glam::uvec3(base, base + 1, base + 2));
}

fn clamped_component<T: Copy>(values: &[T], len: usize) -> Vec<T> {
    values
        .iter()
        .copied()
        .chain(values.last().copied().into_iter().cycle())
        .take(len)
        .collect()
}

fn cone_rotation(
    rotation_axis_angle: Option<&components::RotationAxisAngle>,
    quaternion: Option<&components::RotationQuat>,
) -> glam::Quat {
    let mut rotation = glam::Quat::IDENTITY;
    if let Some(rotation_axis_angle) = rotation_axis_angle
        && let Ok(axis_angle) = glam::Quat::try_from(rotation_axis_angle.0)
    {
        rotation *= axis_angle;
    }
    if let Some(quaternion) = quaternion
        && let Ok(quaternion) = glam::Quat::try_from(quaternion.0)
    {
        rotation *= quaternion;
    }
    rotation
}

impl IdentifiedViewSystem for Cones3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Cones3D"
        )
    }
}

impl VisualizerSystem for Cones3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Length>(
            &Cones3D::descriptor_lengths(),
            &Cones3D::all_components(),
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
            "cones3d",
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Cones3D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_lengths = results.iter_required(Cones3D::descriptor_lengths().component);
                let all_radii = results.iter_optional(Cones3D::descriptor_radii().component);
                let all_centers = results.iter_optional(Cones3D::descriptor_centers().component);
                let all_rotation_axis_angles =
                    results.iter_optional(Cones3D::descriptor_rotation_axis_angles().component);
                let all_quaternions =
                    results.iter_optional(Cones3D::descriptor_quaternions().component);
                let all_colors = results.iter_optional(Cones3D::descriptor_colors().component);
                let all_labels = results.iter_optional(Cones3D::descriptor_labels().component);
                let all_fill_modes =
                    results.iter_optional(Cones3D::descriptor_fill_mode().component);
                let all_albedo_factors =
                    results.iter_optional(Cones3D::descriptor_albedo_factor().component);
                let all_albedo_buffers =
                    results.iter_optional(Cones3D::descriptor_albedo_texture_buffer().component);
                let all_albedo_formats =
                    results.iter_optional(Cones3D::descriptor_albedo_texture_format().component);
                let all_line_radii =
                    results.iter_optional(Cones3D::descriptor_line_radii().component);
                let all_show_labels =
                    results.iter_optional(Cones3D::descriptor_show_labels().component);
                let all_class_ids =
                    results.iter_optional(Cones3D::descriptor_class_ids().component);

                let query_result_hash = results.query_result_hash();
                let texture_data: HashMap<_, _> = re_query::range_zip_1x3(
                    results
                        .iter_required(Cones3D::descriptor_lengths().component)
                        .slice::<f32>(),
                    all_albedo_factors.slice::<u32>(),
                    all_albedo_buffers.slice::<&[u8]>(),
                    all_albedo_formats.component_slow::<ImageFormat>(),
                )
                .map(
                    |(_index, _lengths, albedo_factors, albedo_buffers, albedo_formats)| {
                        (
                            _index,
                            ConeTextureData {
                                albedo_factor: albedo_factors.map(bytemuck::cast_slice).and_then(
                                    |albedo_factors: &[Rgba32]| albedo_factors.first().copied(),
                                ),
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
                        )
                    },
                )
                .collect();

                let data = re_query::range_zip_1x10(
                    all_lengths.slice::<f32>(),
                    all_radii.slice::<f32>(),
                    all_centers.slice::<[f32; 3]>(),
                    all_rotation_axis_angles.component_slow::<components::RotationAxisAngle>(),
                    all_quaternions.slice::<[f32; 4]>(),
                    all_colors.slice::<u32>(),
                    all_line_radii.slice::<f32>(),
                    all_fill_modes.slice::<u8>(),
                    all_labels.slice::<String>(),
                    all_show_labels.slice::<bool>(),
                    all_class_ids.slice::<u16>(),
                )
                .map(
                    |(
                        _index,
                        lengths,
                        radii,
                        centers,
                        rotation_axis_angles,
                        quaternions,
                        colors,
                        line_radii,
                        fill_modes,
                        labels,
                        show_labels,
                        class_ids,
                    )| {
                        let ConeTextureData {
                            albedo_factor,
                            albedo_texture_buffer,
                            albedo_texture_format,
                        } = texture_data.get(&_index).map_or(
                            ConeTextureData {
                                albedo_factor: None,
                                albedo_texture_buffer: None,
                                albedo_texture_format: None,
                            },
                            |texture_data| ConeTextureData {
                                albedo_factor: texture_data.albedo_factor,
                                albedo_texture_buffer: texture_data.albedo_texture_buffer.clone(),
                                albedo_texture_format: texture_data.albedo_texture_format,
                            },
                        );

                        Cones3DComponentData {
                            index: _index,
                            query_result_hash,
                            lengths: bytemuck::cast_slice(lengths),
                            radii: radii.map_or(&[], bytemuck::cast_slice),
                            centers: centers.map_or(&[], bytemuck::cast_slice),
                            rotation_axis_angles: rotation_axis_angles.unwrap_or_default(),
                            quaternions: quaternions.map_or(&[], bytemuck::cast_slice),
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
                            albedo_factor,
                            albedo_texture_buffer,
                            albedo_texture_format,
                        }
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

fn clean_length(suspicious_length: f32) -> f32 {
    if suspicious_length.is_finite() && suspicious_length > 0.0 {
        suspicious_length
    } else {
        0.0
    }
}
