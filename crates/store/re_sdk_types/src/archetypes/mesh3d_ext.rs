use std::collections::BTreeSet;

use arrow::array::{self, Array as _};

use super::Mesh3D;
use crate::archetypes;
use crate::components;

#[derive(thiserror::Error, Debug)]
pub enum Mesh3DError {
    #[error(
        "No indices were specified, so the number of positions must be divisible by 9 [(xyz xyz xyz), …], got {0}"
    )]
    PositionsAreNotTriangles(usize),

    #[error("Index out of bounds: got index={index} with {num_vertices} vertices")]
    IndexOutOfBounds { index: u32, num_vertices: usize },

    #[error(
        "Positions & normals array must have the same length, \
        got positions={0} vs. normals={1}"
    )]
    MismatchedPositionsNormals(usize, usize),
}

impl Mesh3D {
    /// Use this image as the albedo texture.
    #[inline]
    pub fn with_albedo_texture_image(mut self, image: impl Into<archetypes::Image>) -> Self {
        let image = image.into();

        self.albedo_texture_format = image
            .format
            .map(|batch| batch.with_descriptor_override(Self::descriptor_albedo_texture_format()));
        self.albedo_texture_buffer = image
            .buffer
            .map(|batch| batch.with_descriptor_override(Self::descriptor_albedo_texture_buffer()));
        self
    }

    /// Use this image as the albedo texture.
    #[inline]
    pub fn with_albedo_texture(
        self,
        image_format: impl Into<components::ImageFormat>,
        image_buffer: impl Into<components::ImageBuffer>,
    ) -> Self {
        self.with_albedo_texture_format(image_format)
            .with_albedo_texture_buffer(image_buffer)
    }

    /// Check that this is a valid mesh, e.g. that the vertex indices are within bounds
    /// and that we have the same number of positions and normals (if any).
    ///
    /// Only use this when logging a whole new mesh. Not meaningful for field updates!
    #[track_caller]
    pub fn sanity_check(&self) -> Result<(), Mesh3DError> {
        let num_vertices = self.num_vertices();

        let index_data = self.triangle_indices.as_ref().map(|indices| {
            array::as_fixed_size_list_array(&indices.array)
                .values()
                .to_data()
        });

        if let Some(index_data) = index_data {
            for index in index_data.buffer::<u32>(0) {
                if num_vertices <= *index as usize {
                    return Err(Mesh3DError::IndexOutOfBounds {
                        index: *index,
                        num_vertices,
                    });
                }
            }
        } else if !num_vertices.is_multiple_of(9) {
            return Err(Mesh3DError::PositionsAreNotTriangles(num_vertices));
        }

        if let Some(normals) = &self.vertex_normals
            && normals.array.len() != num_vertices
        {
            return Err(Mesh3DError::MismatchedPositionsNormals(
                num_vertices,
                normals.array.len(),
            ));
        }

        Ok(())
    }

    /// The total number of vertices.
    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.vertex_positions
            .as_ref()
            .map_or(0, |positions| positions.array.len())
    }

    /// The total number of triangles.
    #[inline]
    pub fn num_triangles(&self) -> usize {
        if let Some(triangle_indices) = self.triangle_indices.as_ref() {
            triangle_indices.array.len()
        } else {
            self.num_vertices() / 3
        }
    }

    /// Creates a new [`Mesh3D`] from a `.ply` file.
    ///
    /// ## Supported properties
    ///
    /// This expects:
    /// - a `"vertex"` element with required `"x"` and `"y"` properties
    /// - an optional `"z"` vertex property, defaulting to `0.0` when omitted
    /// - a `"face"` element with `"vertex_indices"` or `"vertex_index"` list properties
    ///
    /// Optional vertex properties:
    /// - normals: `"nx"`, `"ny"` & `"nz"`
    /// - colors: `"red"`, `"green"`, `"blue"` & `"alpha"`
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file_path(filepath: &std::path::Path) -> std::io::Result<Self> {
        re_tracing::profile_function!(filepath.to_string_lossy());

        let file = std::fs::File::open(filepath)?;
        let mut file = std::io::BufReader::new(file);
        from_ply_reader(&mut file)
    }

    /// Creates a new [`Mesh3D`] from the contents of a `.ply` file.
    pub fn from_file_contents(contents: &[u8]) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        from_ply_reader(&mut contents)
    }
}

const PROP_X: &str = "x";
const PROP_Y: &str = "y";
const PROP_Z: &str = "z";
const PROP_NX: &str = "nx";
const PROP_NY: &str = "ny";
const PROP_NZ: &str = "nz";
const PROP_RED: &str = "red";
const PROP_GREEN: &str = "green";
const PROP_BLUE: &str = "blue";
const PROP_ALPHA: &str = "alpha";
const PROP_VERTEX_INDEX: &str = "vertex_index";
const PROP_VERTEX_INDICES: &str = "vertex_indices";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlyFaceIndexProperty {
    VertexIndices,
    VertexIndex,
}

fn classify_face_index_property(
    element_def: &ply_rs_bw::ply::ElementDef,
) -> Option<PlyFaceIndexProperty> {
    if element_def.properties.contains_key(PROP_VERTEX_INDICES) {
        Some(PlyFaceIndexProperty::VertexIndices)
    } else if element_def.properties.contains_key(PROP_VERTEX_INDEX) {
        Some(PlyFaceIndexProperty::VertexIndex)
    } else {
        None
    }
}

struct ParsedMeshVertex {
    position: components::Position3D,
    normal: Option<components::Vector3D>,
    color: Option<components::Color>,
}

impl ParsedMeshVertex {
    fn from_props(
        mut props: indexmap::IndexMap<String, ply_rs_bw::ply::Property>,
        ignored_props: &mut BTreeSet<String>,
    ) -> std::io::Result<Self> {
        let (Some(x), Some(y)) = (
            props
                .get(PROP_X)
                .and_then(ply_rs_bw::ply::Property::to_f32_lossy),
            props
                .get(PROP_Y)
                .and_then(ply_rs_bw::ply::Property::to_f32_lossy),
        ) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PLY mesh vertices require \"x\" and \"y\" properties",
            ));
        };

        let z = if props.contains_key(PROP_Z) {
            props
                .get(PROP_Z)
                .and_then(ply_rs_bw::ply::Property::to_f32_lossy)
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "PLY mesh vertex property \"z\" has an unsupported type",
                    )
                })?
        } else {
            0.0
        };

        props.swap_remove(PROP_X);
        props.swap_remove(PROP_Y);
        props.swap_remove(PROP_Z);

        let normal = if let (Some(nx), Some(ny), Some(nz)) = (
            props
                .get(PROP_NX)
                .and_then(ply_rs_bw::ply::Property::to_f32_lossy),
            props
                .get(PROP_NY)
                .and_then(ply_rs_bw::ply::Property::to_f32_lossy),
            props
                .get(PROP_NZ)
                .and_then(ply_rs_bw::ply::Property::to_f32_lossy),
        ) {
            props.swap_remove(PROP_NX);
            props.swap_remove(PROP_NY);
            props.swap_remove(PROP_NZ);
            Some(components::Vector3D::from([nx, ny, nz]))
        } else {
            None
        };

        let color = if let (Some(r), Some(g), Some(b)) = (
            props
                .get(PROP_RED)
                .and_then(ply_rs_bw::ply::Property::to_u8_color_lossy),
            props
                .get(PROP_GREEN)
                .and_then(ply_rs_bw::ply::Property::to_u8_color_lossy),
            props
                .get(PROP_BLUE)
                .and_then(ply_rs_bw::ply::Property::to_u8_color_lossy),
        ) {
            let a = props
                .get(PROP_ALPHA)
                .and_then(ply_rs_bw::ply::Property::to_u8_color_lossy)
                .unwrap_or(255);
            props.swap_remove(PROP_RED);
            props.swap_remove(PROP_GREEN);
            props.swap_remove(PROP_BLUE);
            props.swap_remove(PROP_ALPHA);
            Some(components::Color::new((r, g, b, a)))
        } else {
            None
        };

        for (key, _value) in props {
            ignored_props.insert(key);
        }

        Ok(Self {
            position: components::Position3D::new(x, y, z),
            normal,
            color,
        })
    }
}

fn triangulate_face(indices: &[u32]) -> impl Iterator<Item = components::TriangleIndices> + '_ {
    indices[1..]
        .windows(2)
        .map(|pair| [indices[0], pair[0], pair[1]].into())
}

#[derive(Default)]
struct ParsedMeshFace<const USE_VERTEX_INDICES: bool> {
    indices: Vec<u32>,
}

impl<const USE_VERTEX_INDICES: bool> ParsedMeshFace<USE_VERTEX_INDICES> {
    const fn property_name() -> &'static str {
        if USE_VERTEX_INDICES {
            PROP_VERTEX_INDICES
        } else {
            PROP_VERTEX_INDEX
        }
    }

    fn into_triangle_indices(self) -> Vec<components::TriangleIndices> {
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

fn parse_face(face: ParsedMeshFace<true>) -> Vec<components::TriangleIndices> {
    face.into_triangle_indices()
}

fn parse_face_alias(face: ParsedMeshFace<false>) -> Vec<components::TriangleIndices> {
    face.into_triangle_indices()
}

fn face_unknown_props(element_def: &ply_rs_bw::ply::ElementDef) -> BTreeSet<String> {
    element_def
        .properties
        .keys()
        .filter(|name| !matches!(name.as_str(), PROP_VERTEX_INDICES | PROP_VERTEX_INDEX))
        .cloned()
        .collect()
}

fn missing_face_indices_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "PLY mesh faces require \"vertex_indices\" or \"vertex_index\" list properties",
    )
}

fn from_ply_reader<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<Mesh3D> {
    re_tracing::profile_function!();

    let default_element_parser = ply_rs_bw::parser::Parser::<ply_rs_bw::ply::DefaultElement>::new();
    let face_indices_parser = ply_rs_bw::parser::Parser::<ParsedMeshFace<true>>::new();
    let face_index_parser = ply_rs_bw::parser::Parser::<ParsedMeshFace<false>>::new();

    let (header, face_index_property, face_ignored_props, mut payload_reader) = {
        re_tracing::profile_scope!("read_ply_header");

        let mut payload_reader = ply_rs_bw::parser::Reader::new(reader);
        let header = default_element_parser
            .read_header(&mut payload_reader)
            .map_err(std::io::Error::from)?;
        let face_index_property = header
            .elements
            .get("face")
            .and_then(classify_face_index_property);
        let face_ignored_props = header
            .elements
            .get("face")
            .map(face_unknown_props)
            .unwrap_or_default();

        (
            header,
            face_index_property,
            face_ignored_props,
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
            "vertex" => {
                let vertices = default_element_parser
                    .read_payload_for_element(&mut payload_reader, element_def, &header)
                    .map_err(std::io::Error::from)?;

                for props in vertices {
                    let vertex = ParsedMeshVertex::from_props(props, &mut ignored_props)?;
                    positions.push(vertex.position);
                    normals.push(vertex.normal);
                    colors.push(vertex.color);
                }
            }
            "face" => {
                saw_faces = true;

                match face_index_property {
                    Some(PlyFaceIndexProperty::VertexIndices) => {
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
                    Some(PlyFaceIndexProperty::VertexIndex) => {
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
                        let faces = default_element_parser
                            .read_payload_for_element(&mut payload_reader, element_def, &header)
                            .map_err(std::io::Error::from)?;

                        if !faces.is_empty() {
                            return Err(missing_face_indices_error());
                        }
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

    if !ignored_props.is_empty() {
        re_log::warn!("Ignored properties of .ply file: {ignored_props:?}");
    }

    let mut arch = Mesh3D::new(positions).with_triangle_indices(triangle_indices);

    if normals.iter().any(|normal| normal.is_some()) {
        let normals = normals
            .into_iter()
            .map(|normal| normal.unwrap_or(components::Vector3D::ZERO));
        arch = arch.with_vertex_normals(normals);
    }

    if colors.iter().any(|color| color.is_some()) {
        let colors = colors
            .into_iter()
            .map(|color| color.unwrap_or(components::Color::WHITE));
        arch = arch.with_vertex_colors(colors);
    }

    arch.sanity_check()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

    Ok(arch)
}
