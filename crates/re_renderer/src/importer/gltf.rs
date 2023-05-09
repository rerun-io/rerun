use std::sync::Arc;

use ahash::{HashMap, HashMapExt};
use anyhow::Context as _;
use gltf::texture::WrappingMode;
use smallvec::SmallVec;

use crate::{
    mesh::{Material, Mesh},
    renderer::MeshInstance,
    resource_managers::{
        GpuMeshHandle, GpuTexture2D, ResourceLifeTime, Texture2DCreationDesc, TextureManager2D,
    },
    RenderContext, Rgba32Unmul,
};

/// Loads both gltf and glb into the mesh & texture manager.
pub fn load_gltf_from_buffer(
    mesh_name: &str,
    buffer: &[u8],
    lifetime: ResourceLifeTime,
    ctx: &mut RenderContext,
) -> anyhow::Result<Vec<MeshInstance>> {
    crate::profile_function!();

    let (doc, buffers, images) = {
        crate::profile_scope!("gltf::import_slice");
        gltf::import_slice(buffer)?
    };

    let mut images_as_textures = Vec::with_capacity(images.len());
    for (_index, image) in images.into_iter().enumerate() {
        crate::profile_scope!("image");

        let (format, data) = if let Some(format) = map_format(image.format) {
            (format, image.pixels)
        } else {
            // RGB8 is not supported by wgpu, need to pad out data.
            if image.format == gltf::image::Format::R8G8B8 {
                re_log::debug!("Converting Rgb8 to Rgba8");
                (
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    crate::pad_rgb_to_rgba(&image.pixels, 255),
                )
            } else {
                anyhow::bail!("Unsupported texture format {:?}", image.format);
            }
        };

        // Images don't have names, but textures do. Gather all texture names for debug labeling.
        #[cfg(debug_assertions)]
        let texture_names = doc.textures().fold(String::new(), |mut name_list, t| {
            if t.source().index() == _index {
                if !name_list.is_empty() {
                    name_list.push_str(", ");
                }
                name_list.push_str(t.name().unwrap_or(""));
            }
            name_list
        });
        #[cfg(not(debug_assertions))]
        let texture_names = "";

        let texture = Texture2DCreationDesc {
            label: if texture_names.is_empty() {
                format!("unnamed gltf image in {mesh_name}")
            } else {
                format!("gltf image used by {texture_names} in {mesh_name}")
            }
            .into(),
            data: data.into(),
            format,
            width: image.width,
            height: image.height,
        };

        images_as_textures.push(
            match ctx
                .texture_manager_2d
                .create(&mut ctx.gpu_resources.textures, &texture)
            {
                Ok(texture) => texture,
                Err(err) => {
                    re_log::error!("Failed to create texture: {err}");
                    ctx.texture_manager_2d.white_texture_unorm_handle().clone()
                }
            },
        );
    }

    let mut meshes = HashMap::with_capacity(doc.meshes().len());
    for ref mesh in doc.meshes() {
        crate::profile_scope!("mesh");

        let re_mesh = import_mesh(
            mesh,
            &buffers,
            &images_as_textures,
            &mut ctx.texture_manager_2d,
        )
        .with_context(|| format!("mesh {} (name {:?})", mesh.index(), mesh.name()))?;
        meshes.insert(
            mesh.index(),
            (
                ctx.mesh_manager.write().create(ctx, &re_mesh, lifetime)?,
                Arc::new(re_mesh),
            ),
        );
    }

    let mut instances = Vec::new();
    for scene in doc.scenes() {
        for node in scene.nodes() {
            gather_instances_recursive(&mut instances, &node, &glam::Affine3A::IDENTITY, &meshes);
        }
    }

    Ok(instances)
}

fn map_format(format: gltf::image::Format) -> Option<wgpu::TextureFormat> {
    use gltf::image::Format;
    use wgpu::TextureFormat;

    #[allow(clippy::match_same_arms)]
    match format {
        Format::R8 => Some(TextureFormat::R8Unorm),
        Format::R8G8 => Some(TextureFormat::Rg8Unorm),
        Format::R8G8B8 => None,
        Format::R8G8B8A8 => Some(TextureFormat::Rgba8UnormSrgb),

        Format::R16 => Some(TextureFormat::R16Unorm),
        Format::R16G16 => Some(TextureFormat::Rg16Unorm),
        Format::R16G16B16 => None,
        Format::R16G16B16A16 => Some(TextureFormat::Rgba16Unorm),

        Format::R32G32B32FLOAT => None,
        Format::R32G32B32A32FLOAT => Some(TextureFormat::Rgba32Float),
    }
}

fn import_mesh(
    mesh: &gltf::Mesh<'_>,
    buffers: &[gltf::buffer::Data],
    gpu_image_handles: &[GpuTexture2D],
    texture_manager: &mut TextureManager2D, //imported_materials: HashMap<usize, Material>,
) -> anyhow::Result<Mesh> {
    crate::profile_function!();

    let mut indices = Vec::new();
    let mut vertex_positions = Vec::new();
    let mut vertex_colors = Vec::new();
    let mut vertex_normals = Vec::new();
    let mut vertex_texcoords = Vec::new();
    let mut materials = SmallVec::new();

    // A GLTF mesh consists of several primitives, each with their own material.
    // Primitives map to vertex/index ranges for us as we store all vertices/indices into the same vertex/index buffer.
    // (this means we loose the rarely used ability to re-use vertex/indices between meshes, but shouldn't loose any abilities otherwise)
    for primitive in mesh.primitives() {
        let set = 0;

        let reader = primitive.reader(|buffer| Some(&*buffers[buffer.index()]));

        let index_offset = indices.len() as u32;
        if let Some(primitive_indices) = reader.read_indices() {
            // GLTF restarts the index for every primitive, whereas we use the same range across all materials of the same mesh.
            // (`mesh_renderer` could do this for us by setting a base vertex index)
            let base_index = vertex_positions.len() as u32;
            indices.extend(primitive_indices.into_u32().map(|i| i + base_index));
        } else {
            anyhow::bail!("Gltf primitives must have indices");
        }

        if let Some(primitive_positions) = reader.read_positions() {
            vertex_positions.extend(primitive_positions.map(glam::Vec3::from));
        } else {
            anyhow::bail!("Gltf primitives must have positions");
        }

        if let Some(colors) = reader.read_colors(set) {
            vertex_colors.extend(
                colors
                    .into_rgba_u8()
                    .map(Rgba32Unmul::from_rgba_unmul_array),
            );
        } else {
            vertex_colors.resize(vertex_positions.len(), Rgba32Unmul::WHITE);
        }

        if let Some(primitive_normals) = reader.read_normals() {
            vertex_normals.extend(primitive_normals.map(glam::Vec3::from));
        } else {
            vertex_normals.resize(vertex_positions.len(), glam::Vec3::ZERO);
        }

        if let Some(primitive_texcoords) = reader.read_tex_coords(set) {
            vertex_texcoords.extend(primitive_texcoords.into_f32().map(glam::Vec2::from));
        } else {
            vertex_texcoords.resize(vertex_positions.len(), glam::Vec2::ZERO);
        }

        let primitive_material = primitive.material();
        let pbr_material = primitive_material.pbr_metallic_roughness();

        let albedo = if let Some(texture) = pbr_material.base_color_texture() {
            anyhow::ensure!(
                texture.tex_coord() == 0,
                "Only a single set of texture coordinates is supported"
            );
            let texture = &texture.texture();

            let sampler = &texture.sampler();
            if !matches!(
                sampler.min_filter(),
                None | Some(gltf::texture::MinFilter::LinearMipmapLinear)
            ) || !matches!(
                sampler.mag_filter(),
                None | Some(gltf::texture::MagFilter::Linear)
            ) {
                re_log::warn!(
                    "Textures on meshes are always sampled with a trilinear filter.
 Texture {:?} had {:?} for min and {:?} for mag filtering, these settings will be ignored",
                    texture.name(),
                    sampler.min_filter(),
                    sampler.mag_filter()
                );
            }
            if sampler.wrap_s() != WrappingMode::Repeat || sampler.wrap_t() != WrappingMode::Repeat
            {
                re_log::warn!(
                    "Textures on meshes are always sampled repeating address mode.
 exture {:?} had {:?} for s wrapping and {:?} for t wrapping, these settings will be ignored",
                    texture.name(),
                    sampler.wrap_s(),
                    sampler.wrap_t()
                );
            }

            gpu_image_handles[texture.source().index()].clone()
        } else {
            texture_manager.white_texture_unorm_handle().clone()
        };

        // The color factor *is* in linear space, making things easier for us
        // https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#_material_pbrmetallicroughness_basecolorfactor
        let albedo_factor = {
            let [r, g, b, a] = pbr_material.base_color_factor();
            crate::Rgba::from_rgba_unmultiplied(r, g, b, a)
        };

        materials.push(Material {
            label: primitive.material().name().into(),
            index_range: index_offset..indices.len() as u32,
            albedo,
            albedo_multiplier: albedo_factor,
        });
    }
    if vertex_positions.is_empty() || indices.is_empty() {
        anyhow::bail!("empty mesh");
    }

    let mesh = Mesh {
        label: mesh.name().into(),
        indices,
        vertex_positions,
        vertex_colors,
        vertex_normals,
        vertex_texcoords,
        materials,
    };

    mesh.sanity_check()?;

    Ok(mesh)
}

fn gather_instances_recursive(
    instances: &mut Vec<MeshInstance>,
    node: &gltf::Node<'_>,
    transform: &glam::Affine3A,
    meshes: &HashMap<usize, (GpuMeshHandle, Arc<Mesh>)>,
) {
    let (scale, rotation, translation) = match node.transform() {
        gltf::scene::Transform::Matrix { matrix } => {
            let matrix = glam::Mat4::from_cols_array_2d(&matrix);
            // gltf specifies there that matrices must be described by rotation, scale & translation only.
            matrix.to_scale_rotation_translation()
        }
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => (
            glam::Vec3::from(scale),
            glam::Quat::from_array(rotation),
            glam::Vec3::from(translation),
        ),
    };

    let node_transform =
        glam::Affine3A::from_scale_rotation_translation(scale, rotation, translation);
    let transform = *transform * node_transform;

    for child in node.children() {
        gather_instances_recursive(instances, &child, &transform, meshes);
    }

    if let Some(mesh) = node.mesh() {
        if let Some((gpu_mesh, mesh)) = meshes.get(&mesh.index()) {
            instances.push(MeshInstance {
                gpu_mesh: gpu_mesh.clone(),
                mesh: Some(mesh.clone()),
                world_from_mesh: transform,
                ..Default::default()
            });
        }
    }
}
