use ply_rs_bw::ply::{Property, PropertyAccess, PropertyAccessResult};

use super::Points3D;
use crate::components::{Color, Position3D, Radius, Text};
use crate::ply;

/// The `vertex` properties a `Points3D` is read out of.
///
/// Anything else in the header is reported as ignored, once.
///
/// The gaussian splatting parameters are in here because a `.ply` that carries some of them
/// but is not a full reconstruction (see [`crate::archetypes::GaussianSplats3D`]) still reads
/// as a point cloud: the color comes out of the degree-0 spherical harmonics, and the radius
/// out of the scale.
const SUPPORTED_PROPERTIES: [&str; 16] = [
    ply::PROP_X,
    ply::PROP_Y,
    ply::PROP_Z,
    ply::PROP_RED,
    ply::PROP_GREEN,
    ply::PROP_BLUE,
    ply::PROP_ALPHA,
    ply::PROP_RADIUS,
    ply::PROP_LABEL,
    ply::PROP_SH_DC_0,
    ply::PROP_SH_DC_1,
    ply::PROP_SH_DC_2,
    ply::PROP_OPACITY,
    ply::PROP_SCALE_X,
    ply::PROP_SCALE_Y,
    ply::PROP_SCALE_Z,
];

#[derive(Default)]
struct ParsedPoint3D {
    x: Option<f32>,
    y: Option<f32>,
    z: Option<f32>,
    red: Option<u8>,
    green: Option<u8>,
    blue: Option<u8>,
    alpha: Option<u8>,
    radius: Option<f32>,
    label: Option<Text>,
    sh_dc_0: Option<f32>,
    sh_dc_1: Option<f32>,
    sh_dc_2: Option<f32>,
    opacity: Option<f32>,
    scale_x: Option<f32>,
    scale_y: Option<f32>,
    scale_z: Option<f32>,
}

impl PropertyAccess for ParsedPoint3D {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(&mut self, property_name: &str, property: Property) -> PropertyAccessResult {
        match property_name {
            ply::PROP_X => ply::set_required_f32(&property, &mut self.x),
            ply::PROP_Y => ply::set_required_f32(&property, &mut self.y),
            ply::PROP_Z => ply::set_required_f32(&property, &mut self.z),
            ply::PROP_RED => ply::set_color(&property, &mut self.red),
            ply::PROP_GREEN => ply::set_color(&property, &mut self.green),
            ply::PROP_BLUE => ply::set_color(&property, &mut self.blue),
            ply::PROP_ALPHA => ply::set_color(&property, &mut self.alpha),
            ply::PROP_RADIUS => ply::set_f32(&property, &mut self.radius),
            ply::PROP_LABEL => ply::set_text(&property, &mut self.label),
            ply::PROP_SH_DC_0 => ply::set_f32(&property, &mut self.sh_dc_0),
            ply::PROP_SH_DC_1 => ply::set_f32(&property, &mut self.sh_dc_1),
            ply::PROP_SH_DC_2 => ply::set_f32(&property, &mut self.sh_dc_2),
            ply::PROP_OPACITY => ply::set_f32(&property, &mut self.opacity),
            ply::PROP_SCALE_X => ply::set_f32(&property, &mut self.scale_x),
            ply::PROP_SCALE_Y => ply::set_f32(&property, &mut self.scale_y),
            ply::PROP_SCALE_Z => ply::set_f32(&property, &mut self.scale_z),
            _ => PropertyAccessResult::Ignored,
        }
    }
}

struct Vertex3D {
    position: Position3D,
    color: Option<Color>,
    radius: Option<Radius>,
    label: Option<Text>,
}

impl ParsedPoint3D {
    /// `None` for a vertex with no position, which is nothing we can draw.
    fn into_vertex(self) -> Option<Vertex3D> {
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
            sh_dc_0,
            sh_dc_1,
            sh_dc_2,
            opacity,
            scale_x,
            scale_y,
            scale_z,
        } = self;

        let (Some(x), Some(y)) = (x, y) else {
            return None;
        };

        // `.ply` may leave out `z`, in which case the cloud is flat — the same way the viewer
        // flattens an `x`/`y`-only `.ply` mesh onto `z = 0`.
        let z = z.unwrap_or(0.0);

        // Spherical harmonics win over plain RGB: a file carrying both is a splat.
        let color = if let (Some(r_dc), Some(g_dc), Some(b_dc)) = (sh_dc_0, sh_dc_1, sh_dc_2) {
            fn to_u8(value: f32) -> u8 {
                (value * 255.0 + 0.5) as u8
            }

            // See http://en.wikipedia.org/wiki/Table_of_spherical_harmonics
            let sh_c0 = 0.5 * (1.0 / std::f32::consts::PI).sqrt();
            let alpha = opacity.map_or(255, |value| to_u8(1.0 / (1.0 + (-value).exp())));
            Some(Color::new((
                to_u8(0.5 + sh_c0 * r_dc),
                to_u8(0.5 + sh_c0 * g_dc),
                to_u8(0.5 + sh_c0 * b_dc),
                alpha,
            )))
        } else if let (Some(r), Some(g), Some(b)) = (red, green, blue) {
            Some(Color::new((r, g, b, alpha.unwrap_or(255))))
        } else {
            None
        };

        // Likewise, a gaussian's scale wins over an explicit radius.
        let radius = if let (Some(x), Some(y), Some(z)) = (scale_x, scale_y, scale_z) {
            let (x, y, z) = (x.exp(), y.exp(), z.exp());
            Some(Radius::from((x * y * z).cbrt()))
        } else {
            radius.map(Radius::from)
        };

        Some(Vertex3D {
            position: Position3D::new(x, y, z),
            color,
            radius,
            label,
        })
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
        read_ply(&mut file)
    }

    /// Creates a new [`Points3D`] from the contents of a `.ply` file.
    pub fn from_file_contents(contents: &[u8]) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        read_ply(&mut contents)
    }
}

fn read_ply<T: std::io::BufRead>(reader: &mut T) -> std::io::Result<Points3D> {
    re_tracing::profile_function!();

    let (vertices, _vertex_layout) =
        ply::read_vertex_element::<ParsedPoint3D, _>(reader, &SUPPORTED_PROPERTIES)?;

    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut radii = Vec::new();
    let mut labels = Vec::new();

    for parsed in vertices {
        if let Some(vertex) = parsed.into_vertex() {
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

    re_tracing::profile_scope!("fill-in");

    colors.truncate(positions.len());
    radii.truncate(positions.len());
    labels.truncate(positions.len());

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
