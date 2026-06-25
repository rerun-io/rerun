use std::collections::BTreeSet;

use ply_rs_bw::ply::{Property, PropertyAccess};

use super::Points3D;
use crate::components::{Color, Position3D, Radius, Text};

impl Points3D {
    /// Creates a new [`Points3D`] from a `.ply` file.
    ///
    /// ## Supported properties
    ///
    /// This expects the following property names:
    /// - (Required) Positions of the points: `"x"`, `"y"` & `"z"`.
    /// - (Optional) Colors of the points: `"red"`, `"green"` & `"blue"`.
    /// - (Optional) Radii of the points: `"radius"`.
    /// - (Optional) Labels of the points: `"label"`.
    ///
    /// The media type will be inferred from the path (extension), or the contents if that fails.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file_path(filepath: &std::path::Path) -> std::io::Result<Self> {
        re_tracing::profile_function!(filepath.to_string_lossy());

        let file = std::fs::File::open(filepath)?;
        let mut file = std::io::BufReader::new(file);
        read_ply(&mut file)
    }

    /// Creates a new [`Points3D`] from the contents of a `.ply` file.
    ///
    /// If unspecified, he media type will be inferred from the contents.
    pub fn from_file_contents(contents: &[u8]) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        read_ply(&mut contents)
    }
}

fn f32(prop: &Property) -> Option<f32> {
    match *prop {
        Property::Short(v) => Some(v as f32),
        Property::UShort(v) => Some(v as f32),
        Property::Int(v) => Some(v as f32),
        Property::UInt(v) => Some(v as f32),
        Property::Float(v) => Some(v),
        Property::Double(v) => Some(v as f32),
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

fn u8(prop: &Property) -> Option<u8> {
    match *prop {
        Property::Short(v) => Some(v as u8),
        Property::UShort(v) => Some(v as u8),
        Property::Int(v) => Some(v as u8),
        Property::UInt(v) => Some(v as u8),
        Property::Float(v) => Some((v * 255.0) as u8),
        Property::Double(v) => Some((v * 255.0) as u8),
        Property::Char(v) => Some(v as u8),
        Property::UChar(v) => Some(v),
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

fn string(prop: &Property) -> Option<String> {
    match prop {
        Property::ListUChar(chars) => Some(String::from_utf8_lossy(chars).into_owned()),
        Property::ListChar(_)
        | Property::ListShort(_)
        | Property::ListUShort(_)
        | Property::ListInt(_)
        | Property::ListUInt(_)
        | Property::ListFloat(_)
        | Property::ListDouble(_)
        | Property::Char(_)
        | Property::UChar(_)
        | Property::Short(_)
        | Property::UShort(_)
        | Property::Int(_)
        | Property::UInt(_)
        | Property::Float(_)
        | Property::Double(_) => None,
    }
}

// NOTE: Empirical evidence points to these being de-facto standard…
const PROP_X: &str = "x";
const PROP_Y: &str = "y";
const PROP_Z: &str = "z";
const PROP_RED: &str = "red";
const PROP_GREEN: &str = "green";
const PROP_BLUE: &str = "blue";
const PROP_ALPHA: &str = "alpha";
const PROP_RADIUS: &str = "radius";
const PROP_LABEL: &str = "label";

/// A single vertex, parsed in-place by the PLY parser.
///
/// Implementing [`PropertyAccess`] lets `ply-rs-bw` write each property directly into the relevant
/// field, avoiding the per-vertex `HashMap<String, Property>` allocation of its `DefaultElement`
/// (which is otherwise extremely slow for large point clouds).
#[derive(Default)]
struct Vertex {
    x: f32,
    y: f32,
    z: f32,
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
    alpha: Option<u8>,
    radius: Option<f32>,
    label: Option<String>,
}

impl PropertyAccess for Vertex {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(&mut self, key: &str, property: Property) {
        // Unrecognized properties are ignored here; we warn about them once, up-front,
        // based on the header (see `read_ply`).
        match key {
            PROP_X => self.x = f32(&property).unwrap_or(0.0),
            PROP_Y => self.y = f32(&property).unwrap_or(0.0),
            PROP_Z => self.z = f32(&property).unwrap_or(0.0),
            PROP_RED => self.red = u8(&property),
            PROP_GREEN => self.green = u8(&property),
            PROP_BLUE => self.blue = u8(&property),
            PROP_ALPHA => self.alpha = u8(&property),
            PROP_RADIUS => self.radius = f32(&property),
            PROP_LABEL => self.label = string(&property),
            _ => {}
        }
    }
}

impl Vertex {
    fn position(&self) -> Position3D {
        Position3D::new(self.x, self.y, self.z)
    }

    fn color(&self) -> Option<Color> {
        if let (Some(r), Some(g), Some(b)) = (self.red, self.green, self.blue) {
            Some(Color::new((r, g, b, self.alpha.unwrap_or(255))))
        } else {
            None
        }
    }

    fn radius(&self) -> Option<Radius> {
        self.radius.map(Radius::from)
    }
}

fn read_ply(reader: &mut impl std::io::BufRead) -> std::io::Result<Points3D> {
    re_tracing::profile_function!();

    let parser = ply_rs_bw::parser::Parser::<Vertex>::new();

    let header = {
        re_tracing::profile_scope!("read_header");
        parser.read_header(reader)?
    };

    const KNOWN_PROPS: &[&str] = &[
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

    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut radii = Vec::new();
    let mut labels = Vec::new();

    let mut ignored_props = BTreeSet::new();

    for (key, element) in &header.elements {
        if key == "vertex" {
            let has_positions = [PROP_X, PROP_Y, PROP_Z]
                .iter()
                .all(|p| element.properties.contains_key(*p));
            if !has_positions {
                // All points must have positions; without them there is nothing to read.
                for prop_name in element.properties.keys() {
                    ignored_props.insert(prop_name.clone());
                }
                continue;
            }

            for prop_name in element.properties.keys() {
                if !KNOWN_PROPS.contains(&prop_name.as_str()) {
                    ignored_props.insert(prop_name.clone());
                }
            }

            let vertices = {
                re_tracing::profile_scope!("read_payload");
                parser.read_payload_for_element(reader, element, &header)?
            };

            positions.reserve(vertices.len());
            colors.reserve(vertices.len());
            radii.reserve(vertices.len());
            labels.reserve(vertices.len());

            for vertex in vertices {
                positions.push(vertex.position());
                colors.push(vertex.color());
                radii.push(vertex.radius());
                labels.push(vertex.label.map(|l| Text(l.into())));
            }
        } else {
            re_log::warn!("Ignoring {key:?} in .ply file");
        }
    }

    if !ignored_props.is_empty() {
        re_log::warn!("Ignored properties of .ply file: {ignored_props:?}");
    }

    re_tracing::profile_scope!("fill-in");

    let mut arch = Points3D::new(positions);
    if colors.iter().any(|opt| opt.is_some()) {
        // If some colors have been specified but not others, default the unspecified ones to white.
        let colors = colors
            .into_iter()
            .map(|opt| opt.unwrap_or_else(|| Color::from_rgb(255, 255, 255)));
        arch = arch.with_colors(colors);
    }
    if radii.iter().any(|opt| opt.is_some()) {
        // If some radii have been specified but not others, default the unspecified ones to 1.0.
        let radii = radii
            .into_iter()
            .map(|opt| opt.unwrap_or_else(|| Radius::from(1.0)));
        arch = arch.with_radii(radii);
    }
    if labels.iter().any(|opt| opt.is_some()) {
        // If some labels have been specified but not others, default the unspecified ones to "undef".
        let labels = labels
            .into_iter()
            .map(|opt| opt.unwrap_or(Text("undef".into())));
        arch = arch.with_labels(labels);
    }

    Ok(arch)
}

#[cfg(test)]
mod tests {
    use re_types_core::Loggable as _;

    use super::Points3D;
    use crate::components::{Color, Position3D, Radius, Text};

    fn positions(p: &Points3D) -> Vec<Position3D> {
        p.positions
            .as_ref()
            .map(|c| Position3D::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    fn colors(p: &Points3D) -> Vec<Color> {
        p.colors
            .as_ref()
            .map(|c| Color::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    fn radii(p: &Points3D) -> Vec<Radius> {
        p.radii
            .as_ref()
            .map(|c| Radius::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    fn labels(p: &Points3D) -> Vec<Text> {
        p.labels
            .as_ref()
            .map(|c| Text::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    #[test]
    fn positions_only() {
        let ply = "\
ply
format ascii 1.0
element vertex 2
property float x
property float y
property float z
end_header
1 2 3
4 5 6
";
        let p = Points3D::from_file_contents(ply.as_bytes()).unwrap();
        assert_eq!(
            positions(&p),
            vec![
                Position3D::new(1.0, 2.0, 3.0),
                Position3D::new(4.0, 5.0, 6.0)
            ]
        );
        assert!(colors(&p).is_empty());
        assert!(radii(&p).is_empty());
        assert!(labels(&p).is_empty());
    }

    #[test]
    fn colors_with_alpha_radius_and_ignored_props() {
        // `nx` is an unrecognized property and must be ignored without affecting `x`/`y`/`z`.
        let ply = "\
ply
format ascii 1.0
element vertex 2
property float x
property float y
property float z
property float nx
property uchar red
property uchar green
property uchar blue
property uchar alpha
property float radius
end_header
0 0 0 9 255 0 0 128 0.5
1 1 1 9 0 255 0 64 2.0
";
        let p = Points3D::from_file_contents(ply.as_bytes()).unwrap();
        assert_eq!(
            positions(&p),
            vec![
                Position3D::new(0.0, 0.0, 0.0),
                Position3D::new(1.0, 1.0, 1.0)
            ]
        );
        assert_eq!(
            colors(&p),
            vec![Color::new((255, 0, 0, 128)), Color::new((0, 255, 0, 64))]
        );
        assert_eq!(radii(&p), vec![Radius::from(0.5), Radius::from(2.0)]);
    }

    #[test]
    fn rgb_without_alpha_defaults_to_opaque() {
        let ply = "\
ply
format ascii 1.0
element vertex 1
property float x
property float y
property float z
property uchar red
property uchar green
property uchar blue
end_header
0 0 0 10 20 30
";
        let p = Points3D::from_file_contents(ply.as_bytes()).unwrap();
        assert_eq!(colors(&p), vec![Color::new((10, 20, 30, 255))]);
    }

    #[test]
    fn radii_and_list_labels() {
        // First vertex fully specified, second vertex omits color/radius/label.
        let ply = concat!(
            "ply\n",
            "format ascii 1.0\n",
            "element vertex 2\n",
            "property float x\n",
            "property float y\n",
            "property float z\n",
            "property uchar red\n",
            "property uchar green\n",
            "property uchar blue\n",
            "property float radius\n",
            "property list uchar uchar label\n", // NOLINT
            "end_header\n",
            "0 0 0 1 2 3 0.5 1 65\n",
            "1 1 1 0 0 0 0.0 0\n",
        );
        let p = Points3D::from_file_contents(ply.as_bytes()).unwrap();
        // Second vertex has color (0,0,0) explicitly — both have colors, so no defaulting.
        assert_eq!(
            colors(&p),
            vec![Color::new((1, 2, 3, 255)), Color::new((0, 0, 0, 255))]
        );
        assert_eq!(radii(&p), vec![Radius::from(0.5), Radius::from(0.0)]);
        // Label "A" (65) on first, empty list on second → empty string (the list is present).
        assert_eq!(labels(&p), vec![Text("A".into()), Text("".into())]);
    }

    #[test]
    fn binary_little_endian() {
        let mut ply = b"\
ply
format binary_little_endian 1.0
element vertex 1
property float x
property float y
property float z
end_header
"
        .to_vec();
        ply.extend_from_slice(&1.0f32.to_le_bytes());
        ply.extend_from_slice(&2.0f32.to_le_bytes());
        ply.extend_from_slice(&3.0f32.to_le_bytes());
        let p = Points3D::from_file_contents(&ply).unwrap();
        assert_eq!(positions(&p), vec![Position3D::new(1.0, 2.0, 3.0)]);
    }
}
