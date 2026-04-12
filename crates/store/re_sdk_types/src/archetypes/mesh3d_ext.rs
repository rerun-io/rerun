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
    /// - a `"vertex"` element with required `"x"`, `"y"` and `"z"` properties
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

fn property_to_f32(prop: &ply_rs_bw::ply::Property) -> Option<f32> {
    use ply_rs_bw::ply::Property;

    match prop {
        Property::Short(v) => Some(*v as f32),
        Property::UShort(v) => Some(*v as f32),
        Property::Int(v) => Some(*v as f32),
        Property::UInt(v) => Some(*v as f32),
        Property::Float(v) => Some(*v),
        Property::Double(v) => Some(*v as f32),
        Property::Char(_)
        | Property::UChar(_)
        | Property::ListChar(_)
        | Property::ListUChar(_)
        | Property::ListShort(_)
        | Property::ListUShort(_)
        | Property::ListInt(_)
        | Property::ListUInt(_)
        | Property::ListFloat(_)
        | Property::ListDouble(_) => None,
    }
}

fn property_to_u8(prop: &ply_rs_bw::ply::Property) -> Option<u8> {
    use ply_rs_bw::ply::Property;

    match prop {
        Property::Short(v) => Some(*v as u8),
        Property::UShort(v) => Some(*v as u8),
        Property::Int(v) => Some(*v as u8),
        Property::UInt(v) => Some(*v as u8),
        Property::Float(v) => Some((*v * 255.0) as u8),
        Property::Double(v) => Some((*v * 255.0) as u8),
        Property::Char(v) => Some(*v as u8),
        Property::UChar(v) => Some(*v),
        Property::ListChar(_)
        | Property::ListUChar(_)
        | Property::ListShort(_)
        | Property::ListUShort(_)
        | Property::ListInt(_)
        | Property::ListUInt(_)
        | Property::ListFloat(_)
        | Property::ListDouble(_) => None,
    }
}

fn property_to_indices(prop: &ply_rs_bw::ply::Property) -> Option<Vec<u32>> {
    use ply_rs_bw::ply::Property;

    let collect_signed = |values: &[i32]| {
        values
            .iter()
            .copied()
            .map(u32::try_from)
            .collect::<Result<Vec<_>, _>>()
            .ok()
    };

    let collect_short = |values: &[i16]| {
        values
            .iter()
            .copied()
            .map(u32::try_from)
            .collect::<Result<Vec<_>, _>>()
            .ok()
    };

    let collect_char = |values: &[i8]| {
        values
            .iter()
            .copied()
            .map(u32::try_from)
            .collect::<Result<Vec<_>, _>>()
            .ok()
    };

    match prop {
        Property::ListChar(values) => collect_char(values),
        Property::ListUChar(values) => Some(values.iter().copied().map(u32::from).collect()),
        Property::ListShort(values) => collect_short(values),
        Property::ListUShort(values) => Some(values.iter().copied().map(u32::from).collect()),
        Property::ListInt(values) => collect_signed(values),
        Property::ListUInt(values) => Some(values.clone()),
        Property::Char(_)
        | Property::UChar(_)
        | Property::Short(_)
        | Property::UShort(_)
        | Property::Int(_)
        | Property::UInt(_)
        | Property::Float(_)
        | Property::Double(_)
        | Property::ListFloat(_)
        | Property::ListDouble(_) => None,
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
        let (Some(x), Some(y), Some(z)) = (
            props.get(PROP_X).and_then(property_to_f32),
            props.get(PROP_Y).and_then(property_to_f32),
            props.get(PROP_Z).and_then(property_to_f32),
        ) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PLY mesh vertices require \"x\", \"y\" and \"z\" properties",
            ));
        };

        props.swap_remove(PROP_X);
        props.swap_remove(PROP_Y);
        props.swap_remove(PROP_Z);

        let normal = if let (Some(nx), Some(ny), Some(nz)) = (
            props.get(PROP_NX).and_then(property_to_f32),
            props.get(PROP_NY).and_then(property_to_f32),
            props.get(PROP_NZ).and_then(property_to_f32),
        ) {
            props.swap_remove(PROP_NX);
            props.swap_remove(PROP_NY);
            props.swap_remove(PROP_NZ);
            Some(components::Vector3D::from([nx, ny, nz]))
        } else {
            None
        };

        let color = if let (Some(r), Some(g), Some(b)) = (
            props.get(PROP_RED).and_then(property_to_u8),
            props.get(PROP_GREEN).and_then(property_to_u8),
            props.get(PROP_BLUE).and_then(property_to_u8),
        ) {
            let a = props
                .get(PROP_ALPHA)
                .and_then(property_to_u8)
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

fn parse_face(
    mut props: indexmap::IndexMap<String, ply_rs_bw::ply::Property>,
    ignored_props: &mut BTreeSet<String>,
) -> std::io::Result<Vec<components::TriangleIndices>> {
    let indices = props
        .get(PROP_VERTEX_INDICES)
        .and_then(property_to_indices)
        .or_else(|| props.get(PROP_VERTEX_INDEX).and_then(property_to_indices))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PLY mesh faces require \"vertex_indices\" or \"vertex_index\" list properties",
            )
        })?;

    props.swap_remove(PROP_VERTEX_INDICES);
    props.swap_remove(PROP_VERTEX_INDEX);

    for (key, _value) in props {
        ignored_props.insert(key);
    }

    if indices.len() < 3 {
        return Ok(Vec::new());
    }

    Ok(triangulate_face(&indices).collect())
}

fn from_ply_reader<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<Mesh3D> {
    re_tracing::profile_function!();

    let parser = ply_rs_bw::parser::Parser::<ply_rs_bw::ply::DefaultElement>::new();
    let ply = {
        re_tracing::profile_scope!("read_ply");
        parser.read_ply(reader).map_err(std::io::Error::from)?
    };

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut triangle_indices = Vec::new();
    let mut ignored_props = BTreeSet::new();

    let mut saw_faces = false;

    for (key, all_props) in ply.payload {
        match key.as_str() {
            "vertex" => {
                for props in all_props {
                    let vertex = ParsedMeshVertex::from_props(props, &mut ignored_props)?;
                    positions.push(vertex.position);
                    normals.push(vertex.normal);
                    colors.push(vertex.color);
                }
            }
            "face" => {
                saw_faces = true;
                for props in all_props {
                    triangle_indices.extend(parse_face(props, &mut ignored_props)?);
                }
            }
            _ => {
                re_log::warn!("Ignoring {key:?} in .ply file");
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
