use ply_rs_bw::ply::{Property, PropertyAccess, PropertyAccessResult};

use super::Points2D;
use crate::components::{Color, Position2D, Radius, Text};
use crate::ply;

/// The `vertex` properties a `Points2D` is read out of.
///
/// Anything else in the header is reported as ignored, once.
const SUPPORTED_PROPERTIES: [&str; 8] = [
    ply::PROP_X,
    ply::PROP_Y,
    ply::PROP_RED,
    ply::PROP_GREEN,
    ply::PROP_BLUE,
    ply::PROP_ALPHA,
    ply::PROP_RADIUS,
    ply::PROP_LABEL,
];

#[derive(Default)]
struct ParsedPoint2D {
    x: Option<f32>,
    y: Option<f32>,
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
    alpha: Option<u8>,
    radius: Option<f32>,
    label: Option<Text>,
}

impl PropertyAccess for ParsedPoint2D {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(&mut self, property_name: &str, property: Property) -> PropertyAccessResult {
        match property_name {
            ply::PROP_X => ply::set_required_f32(&property, &mut self.x),
            ply::PROP_Y => ply::set_required_f32(&property, &mut self.y),
            ply::PROP_RED => ply::set_color(&property, &mut self.red),
            ply::PROP_GREEN => ply::set_color(&property, &mut self.green),
            ply::PROP_BLUE => ply::set_color(&property, &mut self.blue),
            ply::PROP_ALPHA => ply::set_color(&property, &mut self.alpha),
            ply::PROP_RADIUS => ply::set_f32(&property, &mut self.radius),
            ply::PROP_LABEL => ply::set_text(&property, &mut self.label),
            _ => PropertyAccessResult::Ignored,
        }
    }
}

struct Vertex2D {
    position: Position2D,
    color: Option<Color>,
    radius: Option<Radius>,
    label: Option<Text>,
}

impl ParsedPoint2D {
    /// `None` for a vertex with no position, which is nothing we can draw.
    fn into_vertex(self) -> Option<Vertex2D> {
        let Self {
            x,
            y,
            red,
            green,
            blue,
            alpha,
            radius,
            label,
        } = self;

        let (Some(x), Some(y)) = (x, y) else {
            return None;
        };

        let color = if let (Some(r), Some(g), Some(b)) = (red, green, blue) {
            Some(Color::new((r, g, b, alpha.unwrap_or(255))))
        } else {
            None
        };

        Some(Vertex2D {
            position: Position2D::new(x, y),
            color,
            radius: radius.map(Radius::from),
            label,
        })
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
        read_ply(&mut file)
    }

    /// Creates a new [`Points2D`] from the contents of a `.ply` file.
    pub fn from_file_contents(contents: &[u8]) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        read_ply(&mut contents)
    }
}

fn read_ply<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<Points2D> {
    re_tracing::profile_function!();

    let (vertices, vertex_layout) =
        ply::read_vertex_element::<ParsedPoint2D, _>(reader, &SUPPORTED_PROPERTIES)?;

    if vertex_layout != ply::PlyVertexLayout::Xy {
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
        if let Some(vertex) = parsed.into_vertex() {
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

    re_tracing::profile_scope!("fill-in");

    colors.truncate(positions.len());
    radii.truncate(positions.len());
    labels.truncate(positions.len());

    let mut arch = Points2D::new(positions);
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
        // If some labels have been specified but not others, leave the rest empty.
        let labels = labels.into_iter().map(Option::unwrap_or_default);
        arch = arch.with_labels(labels);
    }

    Ok(arch)
}
