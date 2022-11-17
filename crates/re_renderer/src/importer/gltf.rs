use ahash::{HashMap, HashMapExt};
use anyhow::Context as _;
use gltf::texture::WrappingMode;
use smallvec::SmallVec;

use crate::{
    mesh::{mesh_vertices::MeshVertexData, Material, Mesh},
    renderer::MeshInstance,
    resource_managers::{
        MeshHandle, MeshManager, ResourceLifeTime, Texture2D, Texture2DHandle, TextureManager2D,
    },
};

use super::to_uniform_scale;

/// Loads both gltf and glb.
pub fn load_gltf_from_buffer(
    mesh_name: &str,
    buffer: &[u8],
    lifetime: ResourceLifeTime,
    mesh_manager: &mut MeshManager,
    texture_manager: &mut TextureManager2D,
) -> anyhow::Result<Vec<MeshInstance>> {
    let (doc, buffers, images) = gltf::import_slice(buffer)?;

    let mut images_as_textures = Vec::with_capacity(images.len());
    for (_index, image) in images.into_iter().enumerate() {
        let (format, data) = if let Some(format) = map_format(image.format) {
            (format, image.pixels)
        } else {
            // RGB8 is not supported by wgpu, need to pad out data.
            if image.format == gltf::image::Format::R8G8B8 {
                re_log::debug!("Converting Rgb8 to Rgba8");
                (
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    Texture2D::convert_rgb8_to_rgba8(&image.pixels),
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

        let mut texture = Texture2D {
            label: if texture_names.is_empty() {
                format!("unnamed gltf image in {mesh_name}")
            } else {
                format!("gltf image used by {texture_names} in {mesh_name}")
            }
            .into(),
            data,
            format,
            width: image.width,
            height: image.height,
        };
        texture.pad_rows_if_necessary();

        images_as_textures.push(texture_manager.store_resource(texture, lifetime));
    }

    let mut meshes = HashMap::with_capacity(doc.meshes().len());
    for ref mesh in doc.meshes() {
        let re_mesh = import_mesh(mesh, &buffers, &images_as_textures, texture_manager)
            .with_context(|| format!("mesh {} (name {:?})", mesh.index(), mesh.name()))?;
        meshes.insert(mesh.index(), mesh_manager.store_resource(re_mesh, lifetime));
    }

    let mut instances = Vec::new();
    for scene in doc.scenes() {
        for node in scene.nodes() {
            gather_instances_recursive(
                &mut instances,
                &node,
                &macaw::Conformal3::IDENTITY,
                &meshes,
            );
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
    images: &[Texture2DHandle],
    texture_manager: &mut TextureManager2D, //imported_materials: HashMap<usize, Material>,
) -> anyhow::Result<Mesh> {
    let mut indices = Vec::new();
    let mut vertex_positions = Vec::new();
    let mut vertex_data = Vec::new();
    let mut materials = SmallVec::new();

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| Some(&*buffers[buffer.index()]));

        if let Some(primitive_positions) = reader.read_positions() {
            vertex_positions.extend(primitive_positions.map(glam::Vec3::from));
        } else {
            anyhow::bail!("Gltf primitives must have positions");
        }
        if let Some(primitive_normals) = reader.read_normals() {
            let to_data = |(p, t)| MeshVertexData {
                normal: glam::Vec3::from(p),
                texcoord: glam::Vec2::from(t),
            };

            if let Some(primitive_texcoords) = reader.read_tex_coords(0) {
                vertex_data.extend(
                    primitive_normals
                        .zip(primitive_texcoords.into_f32())
                        .map(to_data),
                );
            } else {
                vertex_data.extend(
                    primitive_normals
                        .zip(std::iter::repeat([0.0, 0.0]))
                        .map(to_data),
                );
            }
        } else {
            anyhow::bail!("Gltf primitives must have normals");
        }

        let index_offset = indices.len() as u32;
        if let Some(primitive_indices) = reader.read_indices() {
            indices.extend(primitive_indices.into_u32());
        } else {
            anyhow::bail!("Gltf primitives must have indices");
        }

        if vertex_positions.len() != vertex_data.len() {
            anyhow::bail!("Number of positions was not equal number of other vertex data.");
        }

        let primitive_material = primitive.material();
        let albedo = if let Some(texture) = primitive_material
            .pbr_metallic_roughness()
            .base_color_texture()
        {
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

            images[texture.source().index()]
        } else {
            texture_manager.white_texture()
        };

        materials.push(Material {
            label: primitive.material().name().into(),
            index_range: index_offset..indices.len() as u32,
            albedo,
        });
    }
    if vertex_positions.is_empty() || indices.is_empty() {
        anyhow::bail!("empty mesh");
    }

    Ok(Mesh {
        label: mesh.name().into(),
        indices,
        vertex_positions,
        vertex_data,
        materials,
    })
}

fn gather_instances_recursive(
    instances: &mut Vec<MeshInstance>,
    node: &gltf::Node<'_>,
    transform: &macaw::Conformal3,
    meshes: &HashMap<usize, MeshHandle>,
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

    let node_transform = macaw::Conformal3::from_scale_rotation_translation(
        to_uniform_scale(scale),
        rotation,
        translation,
    );
    let transform = transform * node_transform;

    for child in node.children() {
        gather_instances_recursive(instances, &child, &transform, meshes);
    }

    if let Some(mesh) = node.mesh() {
        if let Some(mesh) = meshes.get(&mesh.index()) {
            instances.push(MeshInstance {
                mesh: *mesh,
                world_from_mesh: transform,
                additive_tint_srgb: [0, 0, 0, 0],
            });
        }
    }
}
