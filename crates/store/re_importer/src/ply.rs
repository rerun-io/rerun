//! Classification of a `.ply` file's contents, based on its header alone.
//!
//! `.ply` is versatile enough to hold a 2D or 3D point cloud, or a mesh, so the importer has
//! to look at the header before it can pick an archetype. Payload parsing lives with each
//! consumer, since point clouds and meshes validate very different things.

use std::io;

const ELEMENT_VERTEX: &str = "vertex";
const ELEMENT_FACE: &str = "face";

const PROP_X: &str = "x";
const PROP_Y: &str = "y";
const PROP_Z: &str = "z";

/// What a `.ply` file holds, as far as its header tells us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlyGeometryClass {
    Points2D,
    Points3D,
    MeshOrAsset3D,
}

pub fn classify_geometry_header(header: &ply_rs_bw::ply::Header) -> io::Result<PlyGeometryClass> {
    // Be conservative: any non-empty face element means the file carries topology.
    // The renderer owns the stricter decision of whether the face payload is usable.
    let has_faces = header
        .elements
        .get(ELEMENT_FACE)
        .is_some_and(|element_def| 0 < element_def.count);
    if has_faces {
        return Ok(PlyGeometryClass::MeshOrAsset3D);
    }

    let Some(vertex_element) = header.elements.get(ELEMENT_VERTEX) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PLY file is missing required \"vertex\" element",
        ));
    };

    let properties = &vertex_element.properties;
    match (
        properties.contains_key(PROP_X),
        properties.contains_key(PROP_Y),
        properties.contains_key(PROP_Z),
    ) {
        (true, true, false) => Ok(PlyGeometryClass::Points2D),
        (true, true, true) => Ok(PlyGeometryClass::Points3D),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PLY vertex element must contain at least \"x\" and \"y\"",
        )),
    }
}

pub fn classify_geometry_from_bytes(contents: &[u8]) -> io::Result<PlyGeometryClass> {
    let parser = ply_rs_bw::parser::Parser::<ply_rs_bw::ply::DefaultElement>::new();
    let mut reader =
        ply_rs_bw::parser::Reader::new(std::io::BufReader::new(std::io::Cursor::new(contents)));
    let header = parser.read_header(&mut reader).map_err(io::Error::from)?;

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
    fn missing_vertex_element_is_rejected() {
        let contents = br#"ply
format ascii 1.0
element material 1
property int material_index
end_header
7
"#;

        let err = classify(contents).unwrap_err();
        assert!(
            err.to_string()
                .contains("PLY file is missing required \"vertex\" element")
        );
    }

    #[test]
    fn vertices_without_y_are_rejected() {
        let contents = br#"ply
format ascii 1.0
element vertex 2
property float x
property float z
end_header
1 2
4 5
"#;

        let err = classify(contents).unwrap_err();
        assert!(
            err.to_string()
                .contains("PLY vertex element must contain at least \"x\" and \"y\"")
        );
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
