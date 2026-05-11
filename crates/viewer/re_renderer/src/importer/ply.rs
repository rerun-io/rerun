use std::collections::BTreeSet;

use smallvec::smallvec;

use crate::mesh::{self, CpuMesh};
use crate::{CpuModel, Label, RenderContext, Rgba32Unmul};

#[derive(thiserror::Error, Debug)]
pub enum PlyImportError {
    #[error("Error loading PLY mesh: {0}")]
    PlyIo(#[from] std::io::Error),

    #[error(transparent)]
    MeshError(#[from] mesh::MeshError),
}

/// Load a [PLY .ply file](https://en.wikipedia.org/wiki/PLY_(file_format)) into the mesh manager.
pub fn load_ply_from_buffer(
    mesh_name: &str,
    buffer: &[u8],
    ctx: &RenderContext,
) -> Result<CpuModel, PlyImportError> {
    re_tracing::profile_function!();

    let parsed = parse_ply_mesh_from_buffer(buffer)?;
    let num_indices = parsed.triangle_indices.len() * 3;
    let label = Label::from(mesh_name);

    let material = mesh::Material {
        label: label.clone(),
        index_range: 0..num_indices as u32,
        albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
        albedo_factor: crate::Rgba::WHITE,
    };

    let bbox = crate::util::bounding_box_from_points(parsed.vertex_positions.iter().copied());
    let num_vertices = parsed.vertex_positions.len();
    let mesh = CpuMesh {
        label,
        triangle_indices: parsed.triangle_indices,
        vertex_positions: parsed.vertex_positions,
        vertex_colors: parsed.vertex_colors,
        vertex_normals: parsed.vertex_normals,
        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],
        materials: smallvec![material],
        bbox,
    };

    mesh.sanity_check()?;

    Ok(CpuModel::from_single_mesh(mesh))
}

#[derive(Default)]
struct ParsedMeshVertex {
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    nx: Option<f32>,
    ny: Option<f32>,
    nz: Option<f32>,
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
    alpha: Option<u8>,
}

impl ParsedMeshVertex {
    fn into_parts(self) -> std::io::Result<(glam::Vec3, Option<glam::Vec3>, Option<Rgba32Unmul>)> {
        let (Some(x), Some(y)) = (self.x, self.y) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PLY mesh vertices require \"x\" and \"y\" properties",
            ));
        };

        let position = glam::vec3(x, y, self.z.unwrap_or(0.0));
        let normal = if let (Some(nx), Some(ny), Some(nz)) = (self.nx, self.ny, self.nz) {
            Some(glam::vec3(nx, ny, nz))
        } else {
            None
        };
        let color = if let (Some(red), Some(green), Some(blue)) = (self.red, self.green, self.blue)
        {
            Some(Rgba32Unmul::from_rgba_unmul_array([
                red,
                green,
                blue,
                self.alpha.unwrap_or(255),
            ]))
        } else {
            None
        };

        Ok((position, normal, color))
    }
}

impl ply_rs_bw::ply::PropertyAccess for ParsedMeshVertex {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(
        &mut self,
        property_name: &str,
        property: ply_rs_bw::ply::Property,
    ) -> ply_rs_bw::ply::PropertyAccessResult {
        use ply_rs_bw::ply::PropertyAccessResult;

        match property_name {
            re_ply::PROP_X => {
                if let Some(value) = property.to_f32_lossy() {
                    self.x = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::UnsupportedType
                }
            }
            re_ply::PROP_Y => {
                if let Some(value) = property.to_f32_lossy() {
                    self.y = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::UnsupportedType
                }
            }
            re_ply::PROP_Z => {
                if let Some(value) = property.to_f32_lossy() {
                    self.z = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::UnsupportedType
                }
            }
            re_ply::PROP_NX => {
                if let Some(value) = property.to_f32_lossy() {
                    self.nx = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            re_ply::PROP_NY => {
                if let Some(value) = property.to_f32_lossy() {
                    self.ny = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            re_ply::PROP_NZ => {
                if let Some(value) = property.to_f32_lossy() {
                    self.nz = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            re_ply::PROP_RED => {
                if let Some(value) = property.to_u8_color_lossy() {
                    self.red = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            re_ply::PROP_GREEN => {
                if let Some(value) = property.to_u8_color_lossy() {
                    self.green = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            re_ply::PROP_BLUE => {
                if let Some(value) = property.to_u8_color_lossy() {
                    self.blue = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            re_ply::PROP_ALPHA => {
                if let Some(value) = property.to_u8_color_lossy() {
                    self.alpha = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            _ => PropertyAccessResult::Ignored,
        }
    }
}

fn triangulate_face(indices: &[u32]) -> impl Iterator<Item = glam::UVec3> + '_ {
    indices[1..]
        .windows(2)
        .map(|pair| glam::uvec3(indices[0], pair[0], pair[1]))
}

#[derive(Default)]
struct ParsedMeshFace<const USE_VERTEX_INDICES: bool> {
    indices: Vec<u32>,
}

impl<const USE_VERTEX_INDICES: bool> ParsedMeshFace<USE_VERTEX_INDICES> {
    const fn property_name() -> &'static str {
        if USE_VERTEX_INDICES {
            re_ply::PROP_VERTEX_INDICES
        } else {
            re_ply::PROP_VERTEX_INDEX
        }
    }

    fn into_triangle_indices(self) -> Vec<glam::UVec3> {
        if self.indices.len() < 3 {
            return Vec::new();
        }

        triangulate_face(&self.indices).collect()
    }
}

impl<const USE_VERTEX_INDICES: bool> ply_rs_bw::ply::PropertyAccess
    for ParsedMeshFace<USE_VERTEX_INDICES>
{
    fn new() -> Self {
        Self::default()
    }

    fn set_property(
        &mut self,
        property_name: &str,
        property: ply_rs_bw::ply::Property,
    ) -> ply_rs_bw::ply::PropertyAccessResult {
        use ply_rs_bw::ply::PropertyAccessResult;

        if property_name != Self::property_name() {
            return PropertyAccessResult::Ignored;
        }

        if let Some(indices) = property.to_u32_list() {
            self.indices = indices;
            PropertyAccessResult::Set
        } else {
            PropertyAccessResult::UnsupportedType
        }
    }
}

fn parse_face(face: ParsedMeshFace<true>) -> Vec<glam::UVec3> {
    face.into_triangle_indices()
}

fn parse_face_alias(face: ParsedMeshFace<false>) -> Vec<glam::UVec3> {
    face.into_triangle_indices()
}

fn face_unknown_props(element_def: &ply_rs_bw::ply::ElementDef) -> BTreeSet<String> {
    element_def
        .properties
        .keys()
        .filter(|name| !re_ply::is_mesh_face_index_property(name.as_str()))
        .cloned()
        .collect()
}

fn vertex_unknown_props(element_def: &ply_rs_bw::ply::ElementDef) -> BTreeSet<String> {
    element_def
        .properties
        .keys()
        .filter(|name| !re_ply::is_mesh_vertex_property(name.as_str()))
        .cloned()
        .collect()
}

fn missing_face_indices_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "PLY mesh faces require \"vertex_indices\" or \"vertex_index\" list properties",
    )
}

fn missing_face_topology_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "PLY mesh requires at least one face with 3 or more vertex indices",
    )
}

#[derive(Debug)]
struct ParsedPlyMesh {
    vertex_positions: Vec<glam::Vec3>,
    vertex_normals: Vec<glam::Vec3>,
    vertex_colors: Vec<Rgba32Unmul>,
    triangle_indices: Vec<glam::UVec3>,
}

fn parse_ply_mesh_from_buffer(buffer: &[u8]) -> std::io::Result<ParsedPlyMesh> {
    let mut reader = std::io::Cursor::new(buffer);
    parse_ply_mesh(&mut reader)
}

fn parse_ply_mesh<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<ParsedPlyMesh> {
    re_tracing::profile_function!();

    let default_element_parser = ply_rs_bw::parser::Parser::<ply_rs_bw::ply::DefaultElement>::new();
    let vertex_parser = ply_rs_bw::parser::Parser::<ParsedMeshVertex>::new();
    let face_indices_parser = ply_rs_bw::parser::Parser::<ParsedMeshFace<true>>::new();
    let face_index_parser = ply_rs_bw::parser::Parser::<ParsedMeshFace<false>>::new();

    let (header, face_index_property, face_ignored_props, vertex_ignored_props, mut payload_reader) = {
        re_tracing::profile_scope!("read_ply_header");

        let mut payload_reader = ply_rs_bw::parser::Reader::new(reader);
        let header = default_element_parser
            .read_header(&mut payload_reader)
            .map_err(std::io::Error::from)?;
        let face_index_property = header
            .elements
            .get(re_ply::ELEMENT_FACE)
            .and_then(re_ply::classify_face_index_property);
        let face_ignored_props = header
            .elements
            .get(re_ply::ELEMENT_FACE)
            .map(face_unknown_props)
            .unwrap_or_default();
        let vertex_ignored_props = header
            .elements
            .get(re_ply::ELEMENT_VERTEX)
            .map(vertex_unknown_props)
            .unwrap_or_default();

        (
            header,
            face_index_property,
            face_ignored_props,
            vertex_ignored_props,
            payload_reader,
        )
    };

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut triangle_indices = Vec::new();
    let mut ignored_props = BTreeSet::new();

    let mut saw_faces = false;

    for (_key, element_def) in &header.elements {
        match element_def.name.as_str() {
            re_ply::ELEMENT_VERTEX => {
                let vertices = vertex_parser
                    .read_payload_for_element(&mut payload_reader, element_def, &header)
                    .map_err(std::io::Error::from)?;

                if !vertices.is_empty() {
                    ignored_props.extend(vertex_ignored_props.iter().cloned());
                }

                for vertex in vertices {
                    let (position, normal, color) = vertex.into_parts()?;
                    positions.push(position);
                    normals.push(normal);
                    colors.push(color);
                }
            }
            re_ply::ELEMENT_FACE => {
                saw_faces = true;

                match face_index_property {
                    Some(re_ply::PlyFaceIndexProperty::VertexIndices) => {
                        let faces = face_indices_parser
                            .read_payload_for_element(&mut payload_reader, element_def, &header)
                            .map_err(std::io::Error::from)?;

                        if !faces.is_empty() {
                            ignored_props.extend(face_ignored_props.iter().cloned());
                        }

                        for face in faces {
                            triangle_indices.extend(parse_face(face));
                        }
                    }
                    Some(re_ply::PlyFaceIndexProperty::VertexIndex) => {
                        let faces = face_index_parser
                            .read_payload_for_element(&mut payload_reader, element_def, &header)
                            .map_err(std::io::Error::from)?;

                        if !faces.is_empty() {
                            ignored_props.extend(face_ignored_props.iter().cloned());
                        }

                        for face in faces {
                            triangle_indices.extend(parse_face_alias(face));
                        }
                    }
                    None => {
                        return Err(missing_face_indices_error());
                    }
                }
            }
            _ => {
                re_log::warn!("Ignoring {:?} in .ply file", element_def.name);
                let _ignored = default_element_parser
                    .read_payload_for_element(&mut payload_reader, element_def, &header)
                    .map_err(std::io::Error::from)?;
            }
        }
    }

    if !saw_faces {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "PLY mesh requires a \"face\" element",
        ));
    }

    if triangle_indices.is_empty() {
        return Err(missing_face_topology_error());
    }

    if !ignored_props.is_empty() {
        re_log::warn!("Ignored properties of .ply file: {ignored_props:?}");
    }

    let vertex_normals = if normals.iter().any(Option::is_some) {
        normals
            .into_iter()
            .map(|normal| normal.unwrap_or(glam::Vec3::ZERO))
            .collect()
    } else {
        vec![glam::Vec3::ZERO; positions.len()]
    };

    let vertex_colors = if colors.iter().any(Option::is_some) {
        colors
            .into_iter()
            .map(|color| color.unwrap_or(Rgba32Unmul::WHITE))
            .collect()
    } else {
        vec![Rgba32Unmul::WHITE; positions.len()]
    };

    Ok(ParsedPlyMesh {
        vertex_positions: positions,
        vertex_normals,
        vertex_colors,
        triangle_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ply_parses_mesh_with_normals_colors_and_triangulates_faces() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property float z
property float nx
property float ny
property float nz
property uchar red
property uchar green
property uchar blue
property float temperature
element face 2
property list uchar int vertex_indices
property uchar material_index
end_header
0 0 0 0 0 1 255 0 0 10
1 0 0 0 0 1 0 255 0 11
1 1 0 0 0 1 0 0 255 12
0 1 0 0 0 1 255 255 0 13
3 0 1 2 7
4 0 2 3 1 8
"#;

        let parsed = parse_ply_mesh_from_buffer(contents).unwrap();

        assert_eq!(
            parsed.vertex_positions,
            vec![
                glam::vec3(0.0, 0.0, 0.0),
                glam::vec3(1.0, 0.0, 0.0),
                glam::vec3(1.0, 1.0, 0.0),
                glam::vec3(0.0, 1.0, 0.0),
            ]
        );
        assert_eq!(parsed.vertex_normals, vec![glam::vec3(0.0, 0.0, 1.0); 4]);
        assert_eq!(
            parsed.vertex_colors,
            vec![
                Rgba32Unmul::from_rgb(255, 0, 0),
                Rgba32Unmul::from_rgb(0, 255, 0),
                Rgba32Unmul::from_rgb(0, 0, 255),
                Rgba32Unmul::from_rgb(255, 255, 0),
            ]
        );
        assert_eq!(
            parsed.triangle_indices,
            vec![
                glam::uvec3(0, 1, 2),
                glam::uvec3(0, 2, 3),
                glam::uvec3(0, 3, 1),
            ]
        );
    }

    #[test]
    fn ply_parses_xy_mesh_as_z0_mesh() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 1
property list uchar int vertex_indices
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
4 0 1 2 3
"#;

        let parsed = parse_ply_mesh_from_buffer(contents).unwrap();

        assert_eq!(
            parsed.vertex_positions,
            vec![
                glam::vec3(0.0, 0.0, 0.0),
                glam::vec3(1.0, 0.0, 0.0),
                glam::vec3(1.0, 1.0, 0.0),
                glam::vec3(0.0, 1.0, 0.0),
            ]
        );
        assert_eq!(
            parsed.triangle_indices,
            vec![glam::uvec3(0, 1, 2), glam::uvec3(0, 2, 3)]
        );
    }

    #[test]
    fn ply_parses_mesh_with_vertex_index_alias() {
        let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar int vertex_index
end_header
0 0 0
1 0 0
0 1 0
3 0 1 2
"#;

        let parsed = parse_ply_mesh_from_buffer(contents).unwrap();

        assert_eq!(parsed.triangle_indices, vec![glam::uvec3(0, 1, 2)]);
    }

    #[test]
    fn ply_prefers_vertex_indices_over_vertex_index_when_both_are_present() {
        let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar float vertex_index
property list uchar int vertex_indices
end_header
0 0 0
1 0 0
0 1 0
3 9 8 7 3 0 1 2
"#;

        let parsed = parse_ply_mesh_from_buffer(contents).unwrap();

        assert_eq!(parsed.triangle_indices, vec![glam::uvec3(0, 1, 2)]);
    }

    #[test]
    fn ply_rejects_zero_face_mesh() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 0
property list uchar int vertex_indices
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
"#;

        let err = parse_ply_mesh_from_buffer(contents).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string()
                .contains("at least one face with 3 or more vertex indices")
        );
    }

    #[test]
    fn ply_rejects_zero_face_mesh_without_face_indices() {
        let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 0
property int material_index
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
"#;

        let err = parse_ply_mesh_from_buffer(contents).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string()
                .contains("require \"vertex_indices\" or \"vertex_index\"")
        );
    }

    #[test]
    fn ply_rejects_supported_face_properties_with_unsupported_types() {
        let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar float vertex_indices
end_header
0 0 0
1 0 0
0 1 0
3 0 1 2
"#;

        let err = parse_ply_mesh_from_buffer(contents).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("PLY property 'vertex_indices'"));
    }

    #[test]
    fn ply_rejects_missing_face_element() {
        let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
end_header
0 0 0
1 0 0
0 1 0
"#;

        let err = parse_ply_mesh_from_buffer(contents).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("requires a \"face\" element"));
    }

    #[test]
    fn ply_loads_cpu_model_from_buffer() {
        let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar int vertex_indices
end_header
0 0 0
1 0 0
0 1 0
3 0 1 2
"#;

        let ctx = crate::RenderContext::new_test();
        let model = load_ply_from_buffer("triangle.ply", contents, &ctx).unwrap();

        assert!(!model.bbox().is_nothing());
    }
}
