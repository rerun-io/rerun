//! Shared plumbing for reading [PLY](https://en.wikipedia.org/wiki/PLY_(file_format)) files.
//!
//! `.ply` is versatile enough to hold a 2D or 3D point cloud, a mesh, or a gaussian splat
//! reconstruction, so [`classify_geometry_from_bytes`] decides which archetype a given file
//! should become. The per-archetype parsing lives in each archetype's `_ext` file; what is
//! common to all of them lives here.

use std::collections::BTreeSet;

use crate::components::Text;

pub(crate) const ELEMENT_VERTEX: &str = "vertex";

pub(crate) const PROP_X: &str = "x";
pub(crate) const PROP_Y: &str = "y";
pub(crate) const PROP_Z: &str = "z";
pub(crate) const PROP_RED: &str = "red";
pub(crate) const PROP_GREEN: &str = "green";
pub(crate) const PROP_BLUE: &str = "blue";
pub(crate) const PROP_ALPHA: &str = "alpha";
pub(crate) const PROP_RADIUS: &str = "radius";
pub(crate) const PROP_LABEL: &str = "label";

/// Which of `x`/`y`/`z` a `vertex` element carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlyVertexLayout {
    /// `x` and `y`, but no `z`.
    Xy,

    /// `x`, `y` and `z`.
    Xyz,

    /// Anything else, which we cannot read as a point cloud.
    Other,
}

pub(crate) fn classify_vertex_layout(element_def: &ply_rs_bw::ply::ElementDef) -> PlyVertexLayout {
    let has_x = element_def.properties.contains_key(PROP_X);
    let has_y = element_def.properties.contains_key(PROP_Y);
    let has_z = element_def.properties.contains_key(PROP_Z);

    match (has_x, has_y, has_z) {
        (true, true, false) => PlyVertexLayout::Xy,
        (true, true, true) => PlyVertexLayout::Xyz,
        _ => PlyVertexLayout::Other,
    }
}

const ELEMENT_FACE: &str = "face";

/// The `face` properties that can carry topology.
///
/// `vertex_indices` is what the PLY spec says; `vertex_index` is common enough in the wild
/// that the viewer's mesh importer reads it too, so classification has to agree.
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

    /// Topology, which we hand to the viewer untouched as an `Asset3D`.
    MeshOrAsset3D,
}

/// Does the header carry topology the viewer can actually read?
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

/// Decide which archetype a `.ply` file's contents should become.
///
/// Only the header is read, which is cheap: it stops at `end_header`.
pub fn classify_geometry_from_bytes(contents: &[u8]) -> std::io::Result<PlyGeometryClass> {
    let parser = ply_rs_bw::parser::Parser::<ply_rs_bw::ply::DefaultElement>::new();
    let mut reader =
        ply_rs_bw::parser::Reader::new(std::io::BufReader::new(std::io::Cursor::new(contents)));
    let header = parser
        .read_header(&mut reader)
        .map_err(std::io::Error::from)?;

    classify_geometry_header(&header)
}

fn classify_geometry_header(header: &ply_rs_bw::ply::Header) -> std::io::Result<PlyGeometryClass> {
    // Every shape we support needs vertices, so a mesh without them is rejected here rather
    // than surviving to the viewer as an `Asset3D` that fails on zero vertices.
    let Some(vertex_element) = header.elements.get(ELEMENT_VERTEX) else {
        let elements = header.elements.keys().collect::<Vec<_>>();
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("PLY file is missing required \"vertex\" element, and has only: {elements:?}"),
        ));
    };

    // A splat `.ply` is a point cloud carrying extra properties, so it has to be recognized
    // before the vertex-layout rules below claim it as a plain `Points3D`.
    if crate::archetypes::GaussianSplats3D::is_gaussian_splat_vertex_element(vertex_element) {
        return Ok(PlyGeometryClass::GaussianSplats3D);
    }

    if has_readable_faces(header) {
        return Ok(PlyGeometryClass::MeshOrAsset3D);
    }

    match classify_vertex_layout(vertex_element) {
        PlyVertexLayout::Xy => Ok(PlyGeometryClass::Points2D),
        PlyVertexLayout::Xyz => Ok(PlyGeometryClass::Points3D),
        PlyVertexLayout::Other => {
            let properties = vertex_element.properties.keys().collect::<Vec<_>>();
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "PLY vertex element must contain at least \"x\" and \"y\", but has: {properties:?}"
                ),
            ))
        }
    }
}

/// A payload target for elements that have to be consumed, but whose contents we discard.
#[derive(Default)]
pub(crate) struct IgnoredElement;

impl ply_rs_bw::ply::PropertyAccess for IgnoredElement {
    fn new() -> Self {
        Self
    }
}

// ----------------------------------------------------------------------------
// Reading a `vertex` element into an archetype-specific struct.
//
// Each archetype declares the properties it understands as the fields of its own parse
// struct; anything else in the header is reported as ignored, once, up front.

use ply_rs_bw::ply::{Property, PropertyAccess, PropertyAccessResult};

/// Set a property we cannot do without.
///
/// A type we cannot read is an error rather than a silent skip, since dropping it would
/// leave us with no position at all.
pub(crate) fn set_required_f32(
    property: &Property,
    target: &mut Option<f32>,
) -> PropertyAccessResult {
    if let Some(value) = property.to_f32_lossy() {
        *target = Some(value);
        PropertyAccessResult::Set
    } else {
        PropertyAccessResult::UnsupportedType
    }
}

/// Set an optional property, skipping it if its type is not one we can read.
pub(crate) fn set_f32(property: &Property, target: &mut Option<f32>) -> PropertyAccessResult {
    if let Some(value) = property.to_f32_lossy() {
        *target = Some(value);
        PropertyAccessResult::Set
    } else {
        PropertyAccessResult::Ignored
    }
}

/// Set an optional colour component, skipping it if its type is not one we can read.
pub(crate) fn set_color(property: &Property, target: &mut Option<u8>) -> PropertyAccessResult {
    if let Some(value) = property.to_u8_color_lossy() {
        *target = Some(value);
        PropertyAccessResult::Set
    } else {
        PropertyAccessResult::Ignored
    }
}

/// Set an optional text property, which PLY spells as a list of `uchar`.
pub(crate) fn set_text(property: &Property, target: &mut Option<Text>) -> PropertyAccessResult {
    if let Some(chars) = property.as_list_uchar() {
        *target = Some(Text(String::from_utf8_lossy(chars).to_string().into()));
        PropertyAccessResult::Set
    } else {
        PropertyAccessResult::Ignored
    }
}

/// Warn about the properties of an element that the reading archetype has no use for.
///
/// The header lists every property of the element, so this is decided once, before any
/// payload is read: what is supported is exactly what the struct we read into can hold.
///
/// `is_supported` is a predicate rather than a list because some archetypes accept a family
/// of names, such as the `f_rest_*` spherical harmonics coefficients.
pub(crate) fn warn_about_unsupported_properties(
    element_def: &ply_rs_bw::ply::ElementDef,
    filepath: Option<&std::path::Path>,
    is_supported: impl Fn(&str) -> bool,
) {
    let unsupported = element_def
        .properties
        .keys()
        .map(String::as_str)
        .filter(|name| !is_supported(name))
        .collect::<BTreeSet<_>>();

    if !unsupported.is_empty() {
        // Paths go last, so they are easy to strip when copy-pasting.
        let path_suffix = filepath.map_or_else(String::new, |filepath| {
            format!("\nFile path: {}", filepath.display())
        });
        re_log::warn_once!("Ignored properties of .ply file: {unsupported:?}{path_suffix}"); // NOLINT path at end
    }
}

/// Read the `vertex` element into `V`, consuming and discarding every other element.
///
/// Also returns which of `x`/`y`/`z` the header declared, which is how the caller tells a 2D
/// point cloud from a 3D one.
pub(crate) fn read_vertex_element<V: PropertyAccess, T: std::io::BufRead>(
    reader: &mut T,
    supported_properties: &[&str],
) -> std::io::Result<(Vec<V>, PlyVertexLayout)> {
    re_tracing::profile_function!();

    let ignored_element_parser = ply_rs_bw::parser::Parser::<IgnoredElement>::new();
    let vertex_parser = ply_rs_bw::parser::Parser::<V>::new();

    let mut payload_reader = ply_rs_bw::parser::Reader::new(reader);
    let header = {
        re_tracing::profile_scope!("read_ply_header");
        ignored_element_parser
            .read_header(&mut payload_reader)
            .map_err(std::io::Error::from)?
    };

    let vertex_layout = header
        .elements
        .get(ELEMENT_VERTEX)
        .map_or(PlyVertexLayout::Other, classify_vertex_layout);

    re_tracing::profile_scope!("read_ply_payload");

    let mut vertices = Vec::new();
    for (_key, element_def) in &header.elements {
        if element_def.name == ELEMENT_VERTEX {
            vertices = vertex_parser
                .read_payload_for_element(&mut payload_reader, element_def, &header)
                .map_err(std::io::Error::from)?;

            if !vertices.is_empty() {
                warn_about_unsupported_properties(element_def, None, |name| {
                    supported_properties.contains(&name)
                });
            }
        } else {
            re_log::warn!("Ignoring {:?} in .ply file", element_def.name);
            let _ignored = ignored_element_parser
                .read_payload_for_element(&mut payload_reader, element_def, &header)
                .map_err(std::io::Error::from)?;
        }
    }

    Ok((vertices, vertex_layout))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(contents: &[u8]) -> std::io::Result<PlyGeometryClass> {
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

        let err = classify(contents).unwrap_err().to_string();
        assert!(
            err.contains("PLY file is missing required \"vertex\" element"),
            "{err}"
        );
        // The elements the file *does* have, to make the mismatch obvious.
        assert!(err.contains("material"), "{err}");
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

        let err = classify(contents).unwrap_err().to_string();
        assert!(
            err.contains("PLY vertex element must contain at least \"x\" and \"y\""),
            "{err}"
        );
        // The properties the vertex element *does* have, to make the mismatch obvious.
        assert!(err.contains("\"x\", \"z\""), "{err}");
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
