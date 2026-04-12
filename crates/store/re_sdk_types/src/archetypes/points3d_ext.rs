use std::collections::BTreeSet;

use super::Points3D;
use crate::components::{Color, Position3D, Radius, Text};

const PROP_X: &str = "x";
const PROP_Y: &str = "y";
const PROP_Z: &str = "z";
const PROP_RED: &str = "red";
const PROP_GREEN: &str = "green";
const PROP_BLUE: &str = "blue";
const PROP_ALPHA: &str = "alpha";
const PROP_RADIUS: &str = "radius";
const PROP_LABEL: &str = "label";

const SEEN_X: u16 = 1 << 0;
const SEEN_Y: u16 = 1 << 1;
const SEEN_Z: u16 = 1 << 2;
const SEEN_RED: u16 = 1 << 3;
const SEEN_GREEN: u16 = 1 << 4;
const SEEN_BLUE: u16 = 1 << 5;
const SEEN_ALPHA: u16 = 1 << 6;
const SEEN_RADIUS: u16 = 1 << 7;
const SEEN_LABEL: u16 = 1 << 8;

const SEEN_COLORS: u16 = SEEN_RED | SEEN_GREEN | SEEN_BLUE | SEEN_ALPHA;
const SEEN_ALL_KNOWN_PROPS: u16 = SEEN_X | SEEN_Y | SEEN_Z | SEEN_COLORS | SEEN_RADIUS | SEEN_LABEL;

struct Vertex {
    position: Position3D,
    color: Option<Color>,
    radius: Option<Radius>,
    label: Option<Text>,
}

#[derive(Default)]
struct ParsedVertex {
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
    alpha: Option<u8>,
    radius: Option<f32>,
    label: Option<Text>,
    seen_props: u16,
}

impl ParsedVertex {
    fn note_ignored_props(&self, ignored_props: &mut BTreeSet<String>, mask: u16) {
        for (name, bit) in [
            (PROP_X, SEEN_X),
            (PROP_Y, SEEN_Y),
            (PROP_Z, SEEN_Z),
            (PROP_RED, SEEN_RED),
            (PROP_GREEN, SEEN_GREEN),
            (PROP_BLUE, SEEN_BLUE),
            (PROP_ALPHA, SEEN_ALPHA),
            (PROP_RADIUS, SEEN_RADIUS),
            (PROP_LABEL, SEEN_LABEL),
        ] {
            if self.seen_props & mask & bit != 0 {
                ignored_props.insert(name.to_owned());
            }
        }
    }

    fn into_vertex(self, ignored_props: &mut BTreeSet<String>) -> Option<Vertex> {
        let (Some(x), Some(y), Some(z)) = (self.x, self.y, self.z) else {
            // All points must have positions.
            self.note_ignored_props(ignored_props, SEEN_ALL_KNOWN_PROPS);
            return None;
        };

        let color = if let (Some(r), Some(g), Some(b)) = (self.red, self.green, self.blue) {
            Some(Color::new((r, g, b, self.alpha.unwrap_or(255))))
        } else {
            self.note_ignored_props(ignored_props, SEEN_COLORS);
            None
        };

        if self.radius.is_none() {
            self.note_ignored_props(ignored_props, SEEN_RADIUS);
        }

        if self.label.is_none() {
            self.note_ignored_props(ignored_props, SEEN_LABEL);
        }

        Some(Vertex {
            position: Position3D::new(x, y, z),
            color,
            radius: self.radius.map(Radius::from),
            label: self.label,
        })
    }
}

impl ply_rs_bw::ply::PropertyAccess for ParsedVertex {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(&mut self, property_name: &str, property: ply_rs_bw::ply::Property) {
        match property_name {
            PROP_X => {
                self.seen_props |= SEEN_X;
                self.x = property_to_f32(&property);
            }
            PROP_Y => {
                self.seen_props |= SEEN_Y;
                self.y = property_to_f32(&property);
            }
            PROP_Z => {
                self.seen_props |= SEEN_Z;
                self.z = property_to_f32(&property);
            }
            PROP_RED => {
                self.seen_props |= SEEN_RED;
                self.red = property_to_u8(&property);
            }
            PROP_GREEN => {
                self.seen_props |= SEEN_GREEN;
                self.green = property_to_u8(&property);
            }
            PROP_BLUE => {
                self.seen_props |= SEEN_BLUE;
                self.blue = property_to_u8(&property);
            }
            PROP_ALPHA => {
                self.seen_props |= SEEN_ALPHA;
                self.alpha = property_to_u8(&property);
            }
            PROP_RADIUS => {
                self.seen_props |= SEEN_RADIUS;
                self.radius = property_to_f32(&property);
            }
            PROP_LABEL => {
                self.seen_props |= SEEN_LABEL;
                self.label = property_to_text(property);
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct IgnoredElement;

impl ply_rs_bw::ply::PropertyAccess for IgnoredElement {
    fn new() -> Self {
        Self
    }
}

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
        from_ply_reader(&mut file)
    }

    /// Creates a new [`Points3D`] from the contents of a `.ply` file.
    ///
    /// If unspecified, he media type will be inferred from the contents.
    pub fn from_file_contents(contents: &[u8]) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        from_ply_reader(&mut contents)
    }
}

fn property_to_f32(property: &ply_rs_bw::ply::Property) -> Option<f32> {
    use ply_rs_bw::ply::Property;

    match property {
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

fn property_to_u8(property: &ply_rs_bw::ply::Property) -> Option<u8> {
    use ply_rs_bw::ply::Property;

    match property {
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

fn property_to_text(property: ply_rs_bw::ply::Property) -> Option<Text> {
    match property {
        ply_rs_bw::ply::Property::ListUChar(chars) => {
            Some(Text(String::from_utf8_lossy(&chars).to_string().into()))
        }
        ply_rs_bw::ply::Property::ListChar(_)
        | ply_rs_bw::ply::Property::ListShort(_)
        | ply_rs_bw::ply::Property::ListUShort(_)
        | ply_rs_bw::ply::Property::ListInt(_)
        | ply_rs_bw::ply::Property::ListUInt(_)
        | ply_rs_bw::ply::Property::ListFloat(_)
        | ply_rs_bw::ply::Property::ListDouble(_)
        | ply_rs_bw::ply::Property::Char(_)
        | ply_rs_bw::ply::Property::UChar(_)
        | ply_rs_bw::ply::Property::Short(_)
        | ply_rs_bw::ply::Property::UShort(_)
        | ply_rs_bw::ply::Property::Int(_)
        | ply_rs_bw::ply::Property::UInt(_)
        | ply_rs_bw::ply::Property::Float(_)
        | ply_rs_bw::ply::Property::Double(_) => None,
    }
}

fn from_ply_reader<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<Points3D> {
    re_tracing::profile_function!();

    let vertex_parser = ply_rs_bw::parser::Parser::<ParsedVertex>::new();
    let ignored_parser = ply_rs_bw::parser::Parser::<IgnoredElement>::new();

    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut radii = Vec::new();
    let mut labels = Vec::new();

    let mut ignored_props = BTreeSet::new();

    {
        re_tracing::profile_scope!("read_ply");

        let header = vertex_parser.read_header(reader)?;

        for (key, element_def) in &header.elements {
            if key == "vertex" {
                let parsed_vertices =
                    vertex_parser.read_payload_for_element(reader, element_def, &header)?;

                if !parsed_vertices.is_empty() {
                    ignored_props.extend(
                        element_def
                            .properties
                            .keys()
                            .filter(|name| {
                                !matches!(
                                    name.as_str(),
                                    PROP_X
                                        | PROP_Y
                                        | PROP_Z
                                        | PROP_RED
                                        | PROP_GREEN
                                        | PROP_BLUE
                                        | PROP_ALPHA
                                        | PROP_RADIUS
                                        | PROP_LABEL
                                )
                            })
                            .cloned(),
                    );
                }

                for parsed in parsed_vertices {
                    if let Some(vertex) = parsed.into_vertex(&mut ignored_props) {
                        let Vertex {
                            position,
                            color,
                            radius,
                            label,
                        } = vertex;
                        positions.push(position);
                        colors.push(color); // opt
                        radii.push(radius); // opt
                        labels.push(label); // opt
                    }
                }
            } else {
                re_log::warn!("Ignoring {key:?} in .ply file");
                let _ignored_elements =
                    ignored_parser.read_payload_for_element(reader, element_def, &header)?;
            }
        }
    }

    if !ignored_props.is_empty() {
        re_log::warn!("Ignored properties of .ply file: {ignored_props:?}");
    }

    re_tracing::profile_scope!("fill-in");

    colors.truncate(positions.len());
    radii.truncate(positions.len());
    labels.truncate(positions.len());

    let mut arch = crate::archetypes::Points3D::new(positions);
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
