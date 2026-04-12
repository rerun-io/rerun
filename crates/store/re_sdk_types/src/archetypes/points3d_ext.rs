use std::collections::BTreeSet;

use super::{Points2D, Points3D};
use crate::components::{Color, Position2D, Position3D, Radius, Text};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlyVertexLayout {
    Xy,
    Xyz,
    Other,
}

struct Vertex2D {
    position: Position2D,
    color: Option<Color>,
    radius: Option<Radius>,
    label: Option<Text>,
}

struct Vertex3D {
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
    fn note_ignored_props_for(seen_props: u16, ignored_props: &mut BTreeSet<String>, mask: u16) {
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
            if seen_props & mask & bit != 0 {
                ignored_props.insert(name.to_owned());
            }
        }
    }

    fn into_vertex2d(self, ignored_props: &mut BTreeSet<String>) -> Option<Vertex2D> {
        let Self {
            x,
            y,
            z,
            red,
            green,
            blue,
            alpha,
            radius,
            label,
            seen_props,
        } = self;

        let (Some(x), Some(y)) = (x, y) else {
            // All points must have positions.
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_ALL_KNOWN_PROPS);
            return None;
        };

        if z.is_some() {
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_Z);
        }

        let color = if let (Some(r), Some(g), Some(b)) = (red, green, blue) {
            Some(Color::new((r, g, b, alpha.unwrap_or(255))))
        } else {
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_COLORS);
            None
        };

        if radius.is_none() {
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_RADIUS);
        }

        if label.is_none() {
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_LABEL);
        }

        Some(Vertex2D {
            position: Position2D::new(x, y),
            color,
            radius: radius.map(Radius::from),
            label,
        })
    }

    fn into_vertex3d(self, ignored_props: &mut BTreeSet<String>) -> Option<Vertex3D> {
        let Self {
            x,
            y,
            z,
            red,
            green,
            blue,
            alpha,
            radius,
            label,
            seen_props,
        } = self;

        let (Some(x), Some(y), Some(z)) = (x, y, z) else {
            // All points must have positions.
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_ALL_KNOWN_PROPS);
            return None;
        };

        let color = if let (Some(r), Some(g), Some(b)) = (red, green, blue) {
            Some(Color::new((r, g, b, alpha.unwrap_or(255))))
        } else {
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_COLORS);
            None
        };

        if radius.is_none() {
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_RADIUS);
        }

        if label.is_none() {
            Self::note_ignored_props_for(seen_props, ignored_props, SEEN_LABEL);
        }

        Some(Vertex3D {
            position: Position3D::new(x, y, z),
            color,
            radius: radius.map(Radius::from),
            label,
        })
    }
}

impl ply_rs_bw::ply::PropertyAccess for ParsedVertex {
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
            PROP_X => {
                if let Some(value) = property.to_f32_lossy() {
                    self.seen_props |= SEEN_X;
                    self.x = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::UnsupportedType
                }
            }
            PROP_Y => {
                if let Some(value) = property.to_f32_lossy() {
                    self.seen_props |= SEEN_Y;
                    self.y = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::UnsupportedType
                }
            }
            PROP_Z => {
                if let Some(value) = property.to_f32_lossy() {
                    self.seen_props |= SEEN_Z;
                    self.z = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::UnsupportedType
                }
            }
            PROP_RED => {
                self.seen_props |= SEEN_RED;

                if let Some(value) = property.to_u8_color_lossy() {
                    self.red = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            PROP_GREEN => {
                self.seen_props |= SEEN_GREEN;

                if let Some(value) = property.to_u8_color_lossy() {
                    self.green = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            PROP_BLUE => {
                self.seen_props |= SEEN_BLUE;

                if let Some(value) = property.to_u8_color_lossy() {
                    self.blue = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            PROP_ALPHA => {
                self.seen_props |= SEEN_ALPHA;

                if let Some(value) = property.to_u8_color_lossy() {
                    self.alpha = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            PROP_RADIUS => {
                self.seen_props |= SEEN_RADIUS;

                if let Some(value) = property.to_f32_lossy() {
                    self.radius = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            PROP_LABEL => {
                self.seen_props |= SEEN_LABEL;

                if let Some(value) = property_to_text(&property) {
                    self.label = Some(value);
                    PropertyAccessResult::Set
                } else {
                    PropertyAccessResult::Ignored
                }
            }
            _ => PropertyAccessResult::Ignored,
        }
    }
}

impl Points2D {
    /// Creates a new [`Points2D`] from a `.ply` file.
    ///
    /// ## Supported properties
    ///
    /// This expects the following property names:
    /// - (Required) Positions of the points: `"x"` & `"y"` with no `"z"` property.
    /// - (Optional) Colors of the points: `"red"`, `"green"` & `"blue"`.
    /// - (Optional) Radii of the points: `"radius"`.
    /// - (Optional) Labels of the points: `"label"`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file_path(filepath: &std::path::Path) -> std::io::Result<Self> {
        re_tracing::profile_function!(filepath.to_string_lossy());

        let file = std::fs::File::open(filepath)?;
        let mut file = std::io::BufReader::new(file);
        from_ply_reader_2d(&mut file)
    }

    /// Creates a new [`Points2D`] from the contents of a `.ply` file.
    pub fn from_file_contents(contents: &[u8]) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        from_ply_reader_2d(&mut contents)
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
        from_ply_reader_3d(&mut file)
    }

    /// Creates a new [`Points3D`] from the contents of a `.ply` file.
    ///
    /// If unspecified, he media type will be inferred from the contents.
    pub fn from_file_contents(contents: &[u8]) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        from_ply_reader_3d(&mut contents)
    }
}

fn property_to_text(property: &ply_rs_bw::ply::Property) -> Option<Text> {
    property
        .as_list_uchar()
        .map(|chars| Text(String::from_utf8_lossy(chars).to_string().into()))
}

struct ParsedPly {
    vertex_layout: PlyVertexLayout,
    vertices: Vec<ParsedVertex>,
    ignored_props: BTreeSet<String>,
}

fn classify_vertex_layout(element_def: &ply_rs_bw::ply::ElementDef) -> PlyVertexLayout {
    let has_x = element_def.properties.contains_key(PROP_X);
    let has_y = element_def.properties.contains_key(PROP_Y);
    let has_z = element_def.properties.contains_key(PROP_Z);

    match (has_x, has_y, has_z) {
        (true, true, false) => PlyVertexLayout::Xy,
        (true, true, true) => PlyVertexLayout::Xyz,
        _ => PlyVertexLayout::Other,
    }
}

fn parse_ply_vertices<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<ParsedPly> {
    re_tracing::profile_function!();

    let default_element_parser = ply_rs_bw::parser::Parser::<ply_rs_bw::ply::DefaultElement>::new();
    let vertex_parser = ply_rs_bw::parser::Parser::<ParsedVertex>::new();

    let (vertex_layout, vertex_unknown_props, header, mut payload_reader) = {
        re_tracing::profile_scope!("read_ply_header");

        let mut payload_reader = ply_rs_bw::parser::Reader::new(reader);
        let header = default_element_parser
            .read_header(&mut payload_reader)
            .map_err(std::io::Error::from)?;
        let vertex_layout = header
            .elements
            .get("vertex")
            .map(classify_vertex_layout)
            .unwrap_or(PlyVertexLayout::Other);
        let vertex_unknown_props = header
            .elements
            .get("vertex")
            .map(|element_def| {
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
                    .cloned()
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();

        (vertex_layout, vertex_unknown_props, header, payload_reader)
    };

    let mut ply = ParsedPly {
        vertex_layout,
        vertices: Vec::new(),
        ignored_props: BTreeSet::new(),
    };

    re_tracing::profile_scope!("read_ply_payload");

    for (_key, element_def) in &header.elements {
        if element_def.name == "vertex" {
            let vertices = vertex_parser
                .read_payload_for_element(&mut payload_reader, element_def, &header)
                .map_err(std::io::Error::from)?;

            if !vertices.is_empty() {
                ply.ignored_props
                    .extend(vertex_unknown_props.iter().cloned());
            }

            ply.vertices = vertices;
        } else {
            re_log::warn!("Ignoring {:?} in .ply file", element_def.name);
            let _ignored = default_element_parser
                .read_payload_for_element(&mut payload_reader, element_def, &header)
                .map_err(std::io::Error::from)?;
        }
    }

    Ok(ply)
}

fn from_ply_reader_2d<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<Points2D> {
    re_tracing::profile_function!();

    let ParsedPly {
        vertex_layout,
        vertices,
        mut ignored_props,
    } = parse_ply_vertices(reader)?;

    if vertex_layout != PlyVertexLayout::Xy {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected .ply vertex properties \"x\" and \"y\" without \"z\"",
        ));
    }

    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut radii = Vec::new();
    let mut labels = Vec::new();

    for parsed in vertices {
        if let Some(vertex) = parsed.into_vertex2d(&mut ignored_props) {
            let Vertex2D {
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

    if !ignored_props.is_empty() {
        re_log::warn!("Ignored properties of .ply file: {ignored_props:?}");
    }

    re_tracing::profile_scope!("fill-in");

    colors.truncate(positions.len());
    radii.truncate(positions.len());
    labels.truncate(positions.len());

    let mut arch = crate::archetypes::Points2D::new(positions);
    if colors.iter().any(|opt| opt.is_some()) {
        let colors = colors
            .into_iter()
            .map(|opt| opt.unwrap_or_else(|| Color::from_rgb(255, 255, 255)));
        arch = arch.with_colors(colors);
    }
    if radii.iter().any(|opt| opt.is_some()) {
        let radii = radii
            .into_iter()
            .map(|opt| opt.unwrap_or_else(|| Radius::from(1.0)));
        arch = arch.with_radii(radii);
    }
    if labels.iter().any(|opt| opt.is_some()) {
        let labels = labels
            .into_iter()
            .map(|opt| opt.unwrap_or(Text("undef".into())));
        arch = arch.with_labels(labels);
    }

    Ok(arch)
}

fn from_ply_reader_3d<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<Points3D> {
    re_tracing::profile_function!();

    let ParsedPly {
        vertex_layout: _,
        vertices,
        mut ignored_props,
    } = parse_ply_vertices(reader)?;

    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut radii = Vec::new();
    let mut labels = Vec::new();

    for parsed in vertices {
        if let Some(vertex) = parsed.into_vertex3d(&mut ignored_props) {
            let Vertex3D {
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
