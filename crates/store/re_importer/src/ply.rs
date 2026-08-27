//! Classification of a `.ply` file's contents, based on its header alone.
//!
//! `.ply` is versatile enough to hold a 2D or 3D point cloud, a mesh, or a gaussian splat
//! reconstruction, so the importer has to look at the header before it can pick an archetype.
//! Payload parsing lives with each consumer, since each validates very different things.

use std::io;

use re_sdk_types::archetypes::GaussianSplats3D;

const ELEMENT_VERTEX: &str = "vertex";
const ELEMENT_FACE: &str = "face";

const PROP_X: &str = "x";
const PROP_Y: &str = "y";
const PROP_Z: &str = "z";

/// The `face` properties that can carry topology.
///
/// `vertex_indices` is what the PLY spec says; `vertex_index` is common enough in the wild
/// that the renderer accepts it too, so classification has to agree.
const FACE_INDEX_PROPERTIES: [&str; 2] = ["vertex_indices", "vertex_index"];

/// What a `.ply` file holds, as far as its header tells us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlyGeometryClass {
    /// A 3D Gaussian Splatting reconstruction.
    GaussianSplats3D,

    /// A point cloud whose vertices have no `z`.
    Points2D,

    /// A point cloud whose vertices have `x`, `y` and `z`.
    Points3D,

    /// Topology we hand to the viewer untouched, as an `Asset3D`.
    MeshOrAsset3D,
}

/// Does the header carry topology the renderer can actually read?
///
/// A `face` element with no index property leaves nothing to build a mesh out of. Reading such
/// a file as a point cloud shows the user their vertices; calling it a mesh would hand the
/// viewer an `Asset3D` it can only fail on.
fn has_readable_faces(header: &ply_rs_bw::ply::Header) -> bool {
    header
        .elements
        .get(ELEMENT_FACE)
        .is_some_and(|element_def| {
            0 < element_def.count
                && FACE_INDEX_PROPERTIES
                    .iter()
                    .any(|name| element_def.properties.contains_key(*name))
        })
}

fn classify_geometry_header(header: &ply_rs_bw::ply::Header) -> io::Result<PlyGeometryClass> {
    // Every shape we support needs vertices, so a mesh without them is rejected here rather
    // than surviving to the renderer as an `Asset3D` that fails on zero vertices.
    let Some(vertex_element) = header.elements.get(ELEMENT_VERTEX) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PLY file is missing required \"vertex\" element",
        ));
    };

    if has_readable_faces(header) {
        return Ok(PlyGeometryClass::MeshOrAsset3D);
    }

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
    // A splat `.ply` is a point cloud carrying extra properties, so it has to be recognized
    // before the vertex-layout rules below claim it as a plain `Points3D`.
    if GaussianSplats3D::is_gaussian_splat_ply(contents) {
        return Ok(PlyGeometryClass::GaussianSplats3D);
    }

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
    fn classifies_faces_with_vertex_indices_as_a_mesh() {
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

        assert_eq!(classify(contents).unwrap(), PlyGeometryClass::MeshOrAsset3D);
    }

    /// `vertex_index` is not what the spec says, but it is common enough that the renderer
    /// reads it, so classification has to agree.
    #[test]
    fn classifies_faces_with_the_vertex_index_alias_as_a_mesh() {
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

        assert_eq!(classify(contents).unwrap(), PlyGeometryClass::MeshOrAsset3D);
    }

    /// There is no topology to build a mesh out of, so show the user their vertices rather
    /// than hand the viewer an `Asset3D` it can only fail on.
    #[test]
    fn faces_without_index_properties_fall_back_to_points() {
        let contents = br#"ply
format ascii 1.0
element vertex 1
property float x
property float y
element face 1
property int material_index
end_header
1 2
7
"#;

        assert_eq!(classify(contents).unwrap(), PlyGeometryClass::Points2D);
    }

    #[test]
    fn faces_without_a_vertex_element_are_rejected() {
        let contents = br#"ply
format ascii 1.0
element face 1
property list uchar int vertex_indices
end_header
3 0 1 2
"#;

        let err = classify(contents).unwrap_err();
        assert!(
            err.to_string()
                .contains("PLY file is missing required \"vertex\" element")
        );
    }

    #[test]
    fn classifies_gaussian_splats() {
        let contents = br#"ply
format ascii 1.0
element vertex 1
property float x
property float y
property float z
property float f_dc_0
property float f_dc_1
property float f_dc_2
property float opacity
property float scale_0
property float scale_1
property float scale_2
property float rot_0
property float rot_1
property float rot_2
property float rot_3
end_header
0 0 0 1 1 1 1 0 0 0 1 0 0 0
"#;

        assert_eq!(
            classify(contents).unwrap(),
            PlyGeometryClass::GaussianSplats3D
        );
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
