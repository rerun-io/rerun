use half::f16;
use ply_rs_bw::ply::{Addable as _, ElementDef, Property, PropertyAccess, PropertyDef};

use super::GaussianSplats3D;
use crate::components::{Color, Position3D, RotationQuat, Scale3D, SphericalHarmonics3Rgb};
use crate::datatypes::Quaternion;

/// The names of the PLY properties used by 3D Gaussian Splatting (3DGS) training checkpoints,
/// as produced by the reference INRIA implementation and most tools that followed it.
mod prop {
    pub const X: &str = "x";
    pub const Y: &str = "y";
    pub const Z: &str = "z";

    /// Vestigial normals, always zero in practice. Read nowhere, but expected in the header.
    pub const NORMALS: [&str; 3] = ["nx", "ny", "nz"];

    /// The degree-0 (DC) spherical harmonics coefficients, one per RGB channel.
    pub const F_DC: [&str; 3] = ["f_dc_0", "f_dc_1", "f_dc_2"];

    /// Prefix of the higher-degree spherical harmonics coefficients, `f_rest_0` and up.
    ///
    /// These are stored channel-major: all coefficients of R, then all of G, then all of B.
    pub const F_REST_PREFIX: &str = "f_rest_";

    /// Opacity as a logit; the actual opacity is `sigmoid(opacity)`.
    pub const OPACITY: &str = "opacity";

    /// Per-axis scale as a logarithm; the actual scale is `exp(scale_i)`.
    pub const SCALE: [&str; 3] = ["scale_0", "scale_1", "scale_2"];

    /// Rotation as an unnormalized `wxyz` quaternion.
    pub const ROT: [&str; 4] = ["rot_0", "rot_1", "rot_2", "rot_3"];
}

/// The number of spherical harmonics coefficients (RGB triples) we keep, i.e. degrees 1 through 3.
const NUM_SH_COEFFICIENTS: usize = crate::datatypes::SphericalHarmonics3Rgb::NUM_COEFFICIENTS;

/// Where each value we read lands in [`Splat::values`].
mod slot {
    use super::NUM_SH_COEFFICIENTS;

    pub const X: usize = 0;
    pub const Y: usize = 1;
    pub const Z: usize = 2;

    /// Three consecutive slots, one per RGB channel.
    pub const F_DC: usize = 3;

    pub const OPACITY: usize = 6;

    /// Three consecutive slots, one per axis.
    pub const SCALE: usize = 7;

    /// Four consecutive slots, `wxyz`.
    pub const ROT: usize = 10;

    /// [`NUM_SH_COEFFICIENTS`] consecutive RGB triples, coefficient-major.
    pub const SH: usize = 14;

    pub const NUM: usize = SH + 3 * NUM_SH_COEFFICIENTS;
}

impl GaussianSplats3D {
    /// Creates a new [`GaussianSplats3D`] from a 3D Gaussian Splatting (3DGS) `.ply` file.
    ///
    /// See [`Self::from_ply_file_contents`] for the expected format.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_ply_file_path(filepath: &std::path::Path) -> std::io::Result<Self> {
        re_tracing::profile_function!(filepath.to_string_lossy());

        let file = std::fs::File::open(filepath)?;
        let mut file = std::io::BufReader::new(file);
        read_ply(&mut file, Some(filepath))
    }

    /// Creates a new [`GaussianSplats3D`] from the contents of a 3D Gaussian Splatting (3DGS) `.ply` file.
    ///
    /// This expects the property layout of the reference INRIA implementation
    /// (see [`Self::is_gaussian_splat_ply`]), and converts the raw training parameters
    /// to their natural form:
    /// * `scale_*` (logarithmic) becomes linear [`Scale3D`]
    /// * `opacity` (logit) becomes the alpha of the [`Color`] via the sigmoid
    /// * `f_dc_*` becomes the RGB of the [`Color`] by evaluating the degree-0 spherical harmonic
    /// * `rot_*` (`wxyz`) becomes a normalized `xyzw` [`RotationQuat`]
    /// * `f_rest_*` (channel-major) becomes coefficient-major [`SphericalHarmonics3Rgb`],
    ///   zero-padded if the file is of a lower degree than 3, and truncated if of a higher one
    ///
    /// The file path, if known, is only used to improve warning messages.
    pub fn from_ply_file_contents(
        contents: &[u8],
        filepath: Option<&std::path::Path>,
    ) -> std::io::Result<Self> {
        re_tracing::profile_function!();
        let mut contents = std::io::Cursor::new(contents);
        read_ply(&mut contents, filepath)
    }

    /// Does this look like the contents of a 3D Gaussian Splatting (3DGS) `.ply` file?
    ///
    /// Only the header is inspected, making this cheap enough to use as an up-front check.
    /// Requires positions, `f_dc_*`, `opacity`, `scale_*`, and `rot_*` properties on the
    /// `vertex` element; the higher-degree `f_rest_*` coefficients are optional.
    pub fn is_gaussian_splat_ply(contents: &[u8]) -> bool {
        let mut contents = std::io::Cursor::new(contents);
        let parser = ply_rs_bw::parser::Parser::<Splat>::new();
        let Ok(header) = parser.read_header(&mut contents) else {
            return false;
        };
        header
            .elements
            .get("vertex")
            .is_some_and(has_required_splat_properties)
    }
}

fn has_required_splat_properties(element: &ElementDef) -> bool {
    itertools::chain!(
        &[prop::X, prop::Y, prop::Z, prop::OPACITY],
        &prop::F_DC,
        &prop::SCALE,
        &prop::ROT,
    )
    .all(|p| element.properties.contains_key(*p))
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

/// Which [`slot`] a `.ply` property is read into, if any.
///
/// `num_sh_per_channel` is how many spherical harmonics coefficients the file stores per RGB
/// channel, i.e. the stride of its channel-major `f_rest_*` layout. That is a property of the
/// file, not of us: it can exceed [`NUM_SH_COEFFICIENTS`], in which case we keep the leading
/// coefficients of each channel (the optimal degree-3 approximation) and drop the rest.
fn slot_of(name: &str, num_sh_per_channel: usize) -> Option<usize> {
    if let Some(index) = name.strip_prefix(prop::F_REST_PREFIX) {
        if num_sh_per_channel == 0 {
            return None;
        }
        let index = index.parse::<usize>().ok()?;
        let channel = index / num_sh_per_channel;
        let coefficient = index % num_sh_per_channel;
        (channel < 3 && coefficient < NUM_SH_COEFFICIENTS)
            .then(|| slot::SH + 3 * coefficient + channel)
    } else if let Some(i) = prop::F_DC.iter().position(|p| *p == name) {
        Some(slot::F_DC + i)
    } else if let Some(i) = prop::SCALE.iter().position(|p| *p == name) {
        Some(slot::SCALE + i)
    } else if let Some(i) = prop::ROT.iter().position(|p| *p == name) {
        Some(slot::ROT + i)
    } else {
        match name {
            prop::X => Some(slot::X),
            prop::Y => Some(slot::Y),
            prop::Z => Some(slot::Z),
            prop::OPACITY => Some(slot::OPACITY),
            _ => None,
        }
    }
}

/// The `vertex` element with every property we read renamed to its [`slot`] index.
///
/// Doing this once up-front means [`Splat::set_property`] — which runs once per property per
/// vertex, so hundreds of millions of times for a large file — is a single integer parse rather
/// than a pile of string comparisons. It also transposes the channel-major `f_rest_*` coefficients
/// into our coefficient-major layout for free.
///
/// Properties we ignore keep their original name, which can never be mistaken for a slot index:
/// the PLY grammar requires property names to start with a letter or an underscore.
fn read_plan(element: &ElementDef, num_sh_per_channel: usize) -> ElementDef {
    let mut plan = ElementDef::new(element.name.clone());
    for (name, property) in &element.properties {
        let name =
            slot_of(name, num_sh_per_channel).map_or_else(|| name.clone(), |slot| slot.to_string());
        plan.properties
            .add(PropertyDef::new(name, property.data_type.clone()));
    }
    plan
}

/// A single gaussian, parsed in-place by the PLY parser.
///
/// Implementing [`PropertyAccess`] lets `ply-rs-bw` write each property directly into the relevant
/// slot, avoiding the per-vertex `HashMap<String, Property>` allocation of its `DefaultElement`
/// (which is otherwise extremely slow for large files).
struct Splat {
    values: [f32; slot::NUM],
}

impl Default for Splat {
    fn default() -> Self {
        let mut values = [0.0; slot::NUM];
        values[slot::OPACITY] = f32::INFINITY; // sigmoid(∞) = 1, i.e. fully opaque
        values[slot::ROT] = 1.0; // identity (wxyz)
        Self { values }
    }
}

impl PropertyAccess for Splat {
    fn new() -> Self {
        Self::default()
    }

    fn set_property(&mut self, key: &str, property: Property) {
        // Properties that aren't ours kept a name that never parses as a slot index;
        // we warn about them once, up-front, based on the header (see `read_ply`).
        if let Ok(slot) = key.parse::<usize>()
            && let Some(value) = f32(&property)
            && let Some(destination) = self.values.get_mut(slot)
        {
            *destination = value;
        }
    }
}

impl Splat {
    fn position(&self) -> Position3D {
        Position3D::new(
            self.values[slot::X],
            self.values[slot::Y],
            self.values[slot::Z],
        )
    }

    /// Linear per-axis scale (the PLY stores its logarithm).
    fn scale(&self) -> Scale3D {
        Scale3D::from(std::array::from_fn::<_, 3, _>(|i| {
            self.values[slot::SCALE + i].exp()
        }))
    }

    /// Normalized `xyzw` quaternion (the PLY stores an unnormalized `wxyz` one).
    fn quaternion(&self) -> RotationQuat {
        let [w, x, y, z] = std::array::from_fn::<_, 4, _>(|i| self.values[slot::ROT + i]);
        let norm = (w * w + x * x + y * y + z * z).sqrt();
        if norm > 0.0 {
            RotationQuat(Quaternion::from_xyzw([
                x / norm,
                y / norm,
                z / norm,
                w / norm,
            ]))
        } else {
            RotationQuat(Quaternion::IDENTITY)
        }
    }

    /// Base color from the degree-0 spherical harmonic, with the opacity as alpha.
    fn color(&self) -> Color {
        fn to_u8(f: f32) -> u8 {
            (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
        }

        // See http://en.wikipedia.org/wiki/Table_of_spherical_harmonics
        let sh_c0 = 0.5 * (1.0 / std::f32::consts::PI).sqrt();

        let [r, g, b] =
            std::array::from_fn::<_, 3, _>(|i| to_u8(0.5 + sh_c0 * self.values[slot::F_DC + i]));

        // The opacity is stored as a logit; convert it to alpha via the sigmoid.
        let a = to_u8(1.0 / (1.0 + (-self.values[slot::OPACITY]).exp()));

        Color::new((r, g, b, a))
    }

    /// Higher-degree spherical harmonics coefficients.
    ///
    /// Already coefficient-major and zero-padded: [`read_plan`] took care of the transpose.
    fn sh_coefficients(&self) -> SphericalHarmonics3Rgb {
        let coefficients = std::array::from_fn(|coefficient| {
            std::array::from_fn(|channel| {
                f16::from_f32(self.values[slot::SH + 3 * coefficient + channel])
            })
        });
        crate::datatypes::SphericalHarmonics3Rgb(coefficients).into()
    }
}

/// How many gaussians we parse before converting them and freeing the parsed buffer.
///
/// Reading the payload in batches keeps two things in check:
/// * a bogus `element vertex <huge>` header can't make `ply-rs-bw` pre-allocate — and abort the
///   process on — a buffer sized after a number no actual data has backed up yet
/// * peak memory, since the parsed gaussians are freed as we go instead of being kept alive
///   alongside the archetype we build out of them
const SPLATS_PER_BATCH: usize = 64 * 1024;

fn read_ply(
    reader: &mut impl std::io::BufRead,
    filepath: Option<&std::path::Path>,
) -> std::io::Result<GaussianSplats3D> {
    re_tracing::profile_function!();

    // Appended to warnings; paths go last so they are easy to strip when copy-pasting.
    let path_suffix = filepath.map_or_else(String::new, |filepath| {
        format!("\nFile path: {}", filepath.display())
    });

    let parser = ply_rs_bw::parser::Parser::<Splat>::new();

    let header = {
        re_tracing::profile_scope!("read_header");
        parser.read_header(reader)?
    };

    let mut centers = Vec::new();
    let mut scales = Vec::new();
    let mut quaternions = Vec::new();
    let mut colors = Vec::new();
    let mut sh_coefficients = Vec::new();

    let mut read_any_splats = false;

    for (key, element) in &header.elements {
        if key != "vertex" {
            if read_any_splats {
                // Everything after the gaussians is ignored; we just stop reading here.
                re_log::warn_once!("Ignoring {key:?} in .ply file{path_suffix}"); // NOLINT path at end
                break;
            }
            // Skipping an element means decoding its payload, which we don't do — so anything
            // ahead of the `vertex` element would leave the reader on the wrong bytes.
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unsupported .ply file: {key:?} element comes before the vertex element"),
            ));
        }

        if !has_required_splat_properties(element) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not a 3D Gaussian Splatting .ply file: missing one or more of the \
                 x/y/z, f_dc_*, opacity, scale_*, rot_* vertex properties",
            ));
        }

        let num_f_rest = element
            .properties
            .keys()
            .filter(|p| p.starts_with(prop::F_REST_PREFIX))
            .count();
        let num_sh_per_channel = if num_f_rest % 3 == 0 {
            let num_sh_per_channel = num_f_rest / 3;
            if NUM_SH_COEFFICIENTS < num_sh_per_channel {
                re_log::warn_once!(
                    "Ignoring spherical harmonics coefficients above degree 3: keeping {NUM_SH_COEFFICIENTS} of the {num_sh_per_channel} coefficients per channel ({num_f_rest} f_rest_* properties){path_suffix}"
                );
            }
            num_sh_per_channel
        } else {
            re_log::warn_once!(
                "Ignoring spherical harmonics: expected a multiple of 3 f_rest_* properties (RGB), got {num_f_rest}{path_suffix}"
            );
            0
        };
        let has_sh = 0 < num_sh_per_channel;

        let mut ignored_props = std::collections::BTreeSet::new();
        for prop_name in element.properties.keys() {
            // Dropped `f_rest_*` coefficients are covered by the warning above, and the normals
            // are expected to be present even though we have no use for them.
            let known = prop_name.starts_with(prop::F_REST_PREFIX)
                || prop::NORMALS.contains(&prop_name.as_str())
                || slot_of(prop_name, num_sh_per_channel).is_some();
            if !known {
                ignored_props.insert(prop_name.clone());
            }
        }
        if !ignored_props.is_empty() {
            re_log::warn_once!("Ignored properties of .ply file: {ignored_props:?}{path_suffix}"); // NOLINT path at end
        }

        let mut batch = read_plan(element, num_sh_per_channel);
        let mut remaining = element.count;
        while 0 < remaining {
            batch.count = remaining.min(SPLATS_PER_BATCH);
            remaining -= batch.count;

            let splats = {
                re_tracing::profile_scope!("read_payload");
                parser.read_payload_for_element(reader, &batch, &header)?
            };

            centers.reserve(splats.len());
            scales.reserve(splats.len());
            quaternions.reserve(splats.len());
            colors.reserve(splats.len());
            if has_sh {
                sh_coefficients.reserve(splats.len());
            }

            for splat in splats {
                centers.push(splat.position());
                scales.push(splat.scale());
                quaternions.push(splat.quaternion());
                colors.push(splat.color());
                if has_sh {
                    sh_coefficients.push(splat.sh_coefficients());
                }
            }
        }

        read_any_splats = true;
    }

    if !read_any_splats {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a 3D Gaussian Splatting .ply file: it has no vertex element",
        ));
    }

    let mut arch = GaussianSplats3D::new(centers)
        .with_scales(scales)
        .with_quaternions(quaternions)
        .with_colors(colors);
    if !sh_coefficients.is_empty() {
        arch = arch.with_sh_coefficients(sh_coefficients);
    }

    Ok(arch)
}

#[cfg(test)]
mod tests {
    use re_types_core::Loggable as _;

    use super::{GaussianSplats3D, NUM_SH_COEFFICIENTS};
    use crate::components::{Color, Position3D, RotationQuat, Scale3D, SphericalHarmonics3Rgb};
    use crate::datatypes::Quaternion;

    fn centers(g: &GaussianSplats3D) -> Vec<Position3D> {
        g.centers
            .as_ref()
            .map(|c| Position3D::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    fn scales(g: &GaussianSplats3D) -> Vec<Scale3D> {
        g.scales
            .as_ref()
            .map(|c| Scale3D::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    fn quaternions(g: &GaussianSplats3D) -> Vec<RotationQuat> {
        g.quaternions
            .as_ref()
            .map(|c| RotationQuat::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    fn colors(g: &GaussianSplats3D) -> Vec<Color> {
        g.colors
            .as_ref()
            .map(|c| Color::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    fn sh_coefficients(g: &GaussianSplats3D) -> Vec<SphericalHarmonics3Rgb> {
        g.sh_coefficients
            .as_ref()
            .map(|c| SphericalHarmonics3Rgb::from_arrow(&c.array).unwrap())
            .unwrap_or_default()
    }

    /// A header with the given number of `f_rest_*` properties.
    fn header(num_splats: usize, num_f_rest: usize) -> String {
        use std::fmt::Write as _;

        let mut header = format!(
            "ply\nformat ascii 1.0\nelement vertex {num_splats}\n\
             property float x\nproperty float y\nproperty float z\n\
             property float nx\nproperty float ny\nproperty float nz\n\
             property float f_dc_0\nproperty float f_dc_1\nproperty float f_dc_2\n"
        );
        for i in 0..num_f_rest {
            writeln!(header, "property float f_rest_{i}").ok();
        }
        header.push_str(
            "property float opacity\n\
             property float scale_0\nproperty float scale_1\nproperty float scale_2\n\
             property float rot_0\nproperty float rot_1\nproperty float rot_2\nproperty float rot_3\n\
             end_header\n",
        );
        header
    }

    /// A vertex line with `f_rest_i = i`; everything else is zero, but for an identity rotation.
    fn vertex_line(num_f_rest: usize) -> String {
        use std::fmt::Write as _;

        let mut line = String::from("0 0 0 0 0 0  0 0 0 ");
        for i in 0..num_f_rest {
            write!(line, " {i}").ok();
        }
        line.push_str("  0  0 0 0  1 0 0 0\n");
        line
    }

    #[test]
    fn detection() {
        let splat = header(0, 45);
        assert!(GaussianSplats3D::is_gaussian_splat_ply(splat.as_bytes()));

        let no_f_rest = header(0, 0);
        assert!(GaussianSplats3D::is_gaussian_splat_ply(
            no_f_rest.as_bytes()
        ));

        let plain_point_cloud = "\
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
        assert!(!GaussianSplats3D::is_gaussian_splat_ply(
            plain_point_cloud.as_bytes()
        ));

        assert!(!GaussianSplats3D::is_gaussian_splat_ply(b"not a ply file"));
    }

    #[test]
    fn conversions() {
        // No SH rest coefficients; exercises scale/opacity/quaternion/color conversions.
        let mut ply = header(2, 0);
        // x y z nx ny nz f_dc*3 opacity scale*3 rot*4 (wxyz)
        ply.push_str("1 2 3 0 0 0  0 0 0  0  0 0 0  2 0 0 0\n");
        ply.push_str("4 5 6 0 0 0  10 0 -10  -0.4054651  1 -1 0  0 3 0 4\n");

        let g = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap();
        assert_eq!(
            centers(&g),
            vec![
                Position3D::new(1.0, 2.0, 3.0),
                Position3D::new(4.0, 5.0, 6.0)
            ]
        );

        // scale = exp(raw):
        let s = scales(&g);
        assert_eq!(s[0], Scale3D::from([1.0, 1.0, 1.0]));
        let s1: [f32; 3] = [s[1].0.x(), s[1].0.y(), s[1].0.z()];
        let expected = [1.0f32.exp(), (-1.0f32).exp(), 1.0];
        for (a, b) in std::iter::zip(s1, expected) {
            assert!((a - b).abs() < 1e-6, "{a} vs {b}");
        }

        // quaternion: wxyz → xyzw, normalized. (2,0,0,0) → identity; (0,3,0,4) → (0.6, 0, 0.8, 0).
        assert_eq!(
            quaternions(&g),
            vec![
                RotationQuat(Quaternion::from_xyzw([0.0, 0.0, 0.0, 1.0])),
                RotationQuat(Quaternion::from_xyzw([0.6, 0.0, 0.8, 0.0])),
            ]
        );

        // color: rgb = 0.5 + C0 * f_dc (clamped), alpha = sigmoid(opacity).
        // First: dc=0 → 128, opacity=0 → sigmoid(0)=0.5 → 128.
        // Second: dc=±10 → clamped to 255/0, dc=0 → 128; sigmoid(-0.4054651) = 0.4 → 102.
        assert_eq!(
            colors(&g),
            vec![
                Color::new((128, 128, 128, 128)),
                Color::new((255, 128, 0, 102))
            ]
        );

        assert!(sh_coefficients(&g).is_empty());
    }

    #[test]
    fn sh_transposed_and_zero_padded() {
        // Degree 1: 3 coefficients per channel, stored channel-major in the PLY:
        // [R1 R2 R3  G1 G2 G3  B1 B2 B3]
        let mut ply = header(1, 9);
        ply.push_str("0 0 0 0 0 0  0 0 0  1 2 3 4 5 6 7 8 9  0  0 0 0  1 0 0 0\n");

        let g = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap();
        let sh = sh_coefficients(&g);
        assert_eq!(sh.len(), 1);

        // Coefficient-major: [[R1 G1 B1] [R2 G2 B2] [R3 G3 B3] [0 0 0] …]
        let mut expected = [[half::f16::ZERO; 3]; NUM_SH_COEFFICIENTS];
        for (i, v) in [1.0, 4.0, 7.0, 2.0, 5.0, 8.0, 3.0, 6.0, 9.0]
            .into_iter()
            .enumerate()
        {
            expected[i / 3][i % 3] = half::f16::from_f32(v);
        }
        assert_eq!(sh[0].0.0, expected);
    }

    #[test]
    fn full_degree_3() {
        let num_f_rest = 3 * NUM_SH_COEFFICIENTS;
        let mut ply = header(1, num_f_rest);
        ply.push_str(&vertex_line(num_f_rest));

        let g = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap();
        let sh = sh_coefficients(&g);
        assert_eq!(sh.len(), 1);

        // channel-major input: R = 0..15, G = 15..30, B = 30..45.
        for (coefficient, rgb) in sh[0].0.0.iter().enumerate() {
            for (channel, value) in rgb.iter().enumerate() {
                assert_eq!(
                    *value,
                    half::f16::from_f32((channel * NUM_SH_COEFFICIENTS + coefficient) as f32)
                );
            }
        }
    }

    /// A higher-degree file must be truncated to degree 3, not read at the wrong stride.
    #[test]
    fn higher_degree_is_truncated() {
        // Degree 4: 24 coefficients per channel, so R = 0..24, G = 24..48, B = 48..72.
        let num_sh_per_channel = 24;
        let num_f_rest = 3 * num_sh_per_channel;
        let mut ply = header(1, num_f_rest);
        ply.push_str(&vertex_line(num_f_rest));

        let g = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap();
        let sh = sh_coefficients(&g);
        assert_eq!(sh.len(), 1);

        for (coefficient, rgb) in sh[0].0.0.iter().enumerate() {
            for (channel, value) in rgb.iter().enumerate() {
                assert_eq!(
                    *value,
                    half::f16::from_f32((channel * num_sh_per_channel + coefficient) as f32),
                    "coefficient {coefficient}, channel {channel}"
                );
            }
        }
    }

    /// Not a multiple of three: the channel layout is unknowable, so drop the coefficients.
    #[test]
    fn ragged_f_rest_drops_sh() {
        let mut ply = header(1, 10);
        ply.push_str(&vertex_line(10));
        let g = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap();
        assert!(sh_coefficients(&g).is_empty());
        assert_eq!(centers(&g).len(), 1);
    }

    #[test]
    fn rejects_plain_point_cloud() {
        let ply = "\
ply
format ascii 1.0
element vertex 1
property float x
property float y
property float z
end_header
0 0 0
";
        assert!(GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).is_err());
    }

    #[test]
    fn rejects_missing_vertex_element() {
        let ply = "\
ply
format ascii 1.0
element face 0
property list uchar int vertex_index
end_header
";
        let err = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    /// A header can claim any number of vertices; believing it must not blow up the process.
    #[test]
    fn absurd_vertex_count_errors_instead_of_aborting() {
        let ply = header(100_000_000_000, 0);
        let err = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    /// More gaussians than fit in one batch, to cover the batched payload reading.
    #[test]
    fn multiple_batches() {
        use std::fmt::Write as _;

        let num_splats = super::SPLATS_PER_BATCH + 7;
        let mut ply = header(num_splats, 0);
        for i in 0..num_splats {
            writeln!(ply, "{i} 0 0 0 0 0  0 0 0  0  0 0 0  1 0 0 0").ok();
        }

        let g = GaussianSplats3D::from_ply_file_contents(ply.as_bytes(), None).unwrap();
        let centers = centers(&g);
        assert_eq!(centers.len(), num_splats);
        assert_eq!(
            centers[num_splats - 1],
            Position3D::new((num_splats - 1) as f32, 0.0, 0.0)
        );
    }

    #[test]
    fn binary_little_endian() {
        let mut ply = header(1, 0)
            .replace("format ascii 1.0", "format binary_little_endian 1.0")
            .into_bytes();
        // x y z nx ny nz f_dc*3 opacity scale*3 rot*4
        let values: [f32; 17] = [
            1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
        ];
        for v in values {
            ply.extend_from_slice(&v.to_le_bytes());
        }
        let g = GaussianSplats3D::from_ply_file_contents(&ply, None).unwrap();
        assert_eq!(centers(&g), vec![Position3D::new(1.0, 2.0, 3.0)]);
    }
}
