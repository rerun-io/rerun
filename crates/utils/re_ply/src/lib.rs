//! Shared helpers for PLY header classification.
//!
//! This crate intentionally only centralizes PLY names and lightweight header
//! classification. Payload parsing remains with each caller because point
//! clouds, dataloader routing, and mesh rendering have different validation
//! requirements.

use std::io;

pub const ELEMENT_VERTEX: &str = "vertex";
pub const ELEMENT_FACE: &str = "face";

pub const PROP_X: &str = "x";
pub const PROP_Y: &str = "y";
pub const PROP_Z: &str = "z";
pub const PROP_NX: &str = "nx";
pub const PROP_NY: &str = "ny";
pub const PROP_NZ: &str = "nz";
pub const PROP_RED: &str = "red";
pub const PROP_GREEN: &str = "green";
pub const PROP_BLUE: &str = "blue";
pub const PROP_ALPHA: &str = "alpha";
pub const PROP_RADIUS: &str = "radius";
pub const PROP_LABEL: &str = "label";
pub const PROP_VERTEX_INDEX: &str = "vertex_index";
pub const PROP_VERTEX_INDICES: &str = "vertex_indices";

pub const POINT_VERTEX_PROPERTIES: [&str; 9] = [
    PROP_X,
    PROP_Y,
    PROP_Z,
    PROP_RED,
    PROP_GREEN,
    PROP_BLUE,
    PROP_ALPHA,
    PROP_RADIUS,
    PROP_LABEL,
];

pub const MESH_VERTEX_PROPERTIES: [&str; 10] = [
    PROP_X, PROP_Y, PROP_Z, PROP_NX, PROP_NY, PROP_NZ, PROP_RED, PROP_GREEN, PROP_BLUE, PROP_ALPHA,
];

pub const MESH_FACE_INDEX_PROPERTIES: [&str; 2] = [PROP_VERTEX_INDICES, PROP_VERTEX_INDEX];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlyVertexLayout {
    Xy,
    Xyz,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlyFaceIndexProperty {
    VertexIndices,
    VertexIndex,
}

impl PlyFaceIndexProperty {
    #[inline]
    pub const fn property_name(self) -> &'static str {
        match self {
            Self::VertexIndices => PROP_VERTEX_INDICES,
            Self::VertexIndex => PROP_VERTEX_INDEX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlyGeometryClass {
    Points2D,
    Points3D,
    MeshOrAsset3D,
}

#[inline]
pub fn is_point_vertex_property(property_name: &str) -> bool {
    POINT_VERTEX_PROPERTIES.contains(&property_name)
}

#[inline]
pub fn is_mesh_vertex_property(property_name: &str) -> bool {
    MESH_VERTEX_PROPERTIES.contains(&property_name)
}

#[inline]
pub fn is_mesh_face_index_property(property_name: &str) -> bool {
    MESH_FACE_INDEX_PROPERTIES.contains(&property_name)
}

pub fn classify_vertex_layout(element_def: &ply_rs_bw::ply::ElementDef) -> PlyVertexLayout {
    let has_x = element_def.properties.contains_key(PROP_X);
    let has_y = element_def.properties.contains_key(PROP_Y);
    let has_z = element_def.properties.contains_key(PROP_Z);

    match (has_x, has_y, has_z) {
        (true, true, false) => PlyVertexLayout::Xy,
        (true, true, true) => PlyVertexLayout::Xyz,
        _ => PlyVertexLayout::Other,
    }
}

pub fn classify_face_index_property(
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

#[inline]
pub fn has_non_empty_face_element(header: &ply_rs_bw::ply::Header) -> bool {
    header
        .elements
        .get(ELEMENT_FACE)
        .is_some_and(|element_def| element_def.count > 0)
}

pub fn classify_geometry_header(header: &ply_rs_bw::ply::Header) -> io::Result<PlyGeometryClass> {
    // Be conservative: any non-empty face element means the file carries topology.
    // The renderer owns the stricter decision of whether the face payload is usable.
    if has_non_empty_face_element(header) {
        return Ok(PlyGeometryClass::MeshOrAsset3D);
    }

    let Some(vertex_element) = header.elements.get(ELEMENT_VERTEX) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PLY file is missing required \"vertex\" element",
        ));
    };

    match classify_vertex_layout(vertex_element) {
        PlyVertexLayout::Xy => Ok(PlyGeometryClass::Points2D),
        PlyVertexLayout::Xyz => Ok(PlyGeometryClass::Points3D),
        PlyVertexLayout::Other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PLY vertex element must contain at least \"x\" and \"y\"",
        )),
    }
}

pub fn read_header_from_bytes(contents: &[u8]) -> io::Result<ply_rs_bw::ply::Header> {
    let parser = ply_rs_bw::parser::Parser::<ply_rs_bw::ply::DefaultElement>::new();
    let mut reader =
        ply_rs_bw::parser::Reader::new(std::io::BufReader::new(std::io::Cursor::new(contents)));

    parser.read_header(&mut reader).map_err(io::Error::from)
}

pub fn classify_geometry_from_bytes(contents: &[u8]) -> io::Result<PlyGeometryClass> {
    let header = read_header_from_bytes(contents)?;
    classify_geometry_header(&header)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(contents: &[u8]) -> io::Result<PlyGeometryClass> {
        classify_geometry_from_bytes(contents)
    }

    #[test]
    fn classifies_xy_points() {
        let contents = br#"ply
format ascii 1.0
element vertex 1
property float x
property float y
end_header
1 2
"#;

        assert_eq!(classify(contents).unwrap(), PlyGeometryClass::Points2D);
    }

    #[test]
    fn classifies_xyz_points() {
        let contents = br#"ply
format ascii 1.0
element vertex 1
property float x
property float y
property float z
end_header
1 2 3
"#;

        assert_eq!(classify(contents).unwrap(), PlyGeometryClass::Points3D);
    }

    #[test]
    fn non_empty_face_element_is_mesh_even_without_index_properties() {
        let contents = br#"ply
format ascii 1.0
element face 1
property int material_index
end_header
7
"#;

        assert_eq!(classify(contents).unwrap(), PlyGeometryClass::MeshOrAsset3D);
    }

    #[test]
    fn zero_face_element_keeps_point_classification() {
        let contents = br#"ply
format ascii 1.0
element vertex 1
property float x
property float y
element face 0
property list uchar int vertex_indices
end_header
1 2
"#;

        assert_eq!(classify(contents).unwrap(), PlyGeometryClass::Points2D);
    }
}
