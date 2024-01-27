use std::collections::BTreeSet;

use crate::{
    components::{HalfSizes3D, Rotation3D},
    datatypes::Quaternion,
};

use super::Points3D;

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
    pub fn from_file_path(filepath: &std::path::Path) -> anyhow::Result<Self> {
        re_tracing::profile_function!(filepath.to_string_lossy());
        use anyhow::Context as _;

        let file = std::fs::File::open(filepath)
            .with_context(|| format!("Failed to open file {filepath:?}"))?;
        let mut file = std::io::BufReader::new(file);

        let parser = ply_rs::parser::Parser::<ply_rs::ply::DefaultElement>::new();
        let ply = {
            re_tracing::profile_scope!("read_ply");
            parser.read_ply(&mut file)?
        };

        Ok(from_ply(ply))
    }

    /// Creates a new [`Points3D`] from the contents of a `.ply` file.
    ///
    /// If unspecified, he media type will be inferred from the contents.
    pub fn from_file_contents(contents: &[u8]) -> anyhow::Result<Self> {
        re_tracing::profile_function!();
        let parser = ply_rs::parser::Parser::<ply_rs::ply::DefaultElement>::new();
        let mut contents = std::io::Cursor::new(contents);
        let ply = {
            re_tracing::profile_scope!("read_ply");
            parser.read_ply(&mut contents)?
        };
        Ok(from_ply(ply))
    }
}

fn from_ply(ply: ply_rs::ply::Ply<ply_rs::ply::DefaultElement>) -> Points3D {
    re_tracing::profile_function!();

    use std::borrow::Cow;

    use linked_hash_map::LinkedHashMap;
    use ply_rs::ply::Property;

    use crate::components::{Color, Position3D, Radius, Text};

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

    fn string(prop: &Property) -> Option<Cow<'_, str>> {
        match prop {
            Property::ListUChar(chars) => Some(String::from_utf8_lossy(chars)),
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

    struct Vertex {
        position: Position3D,
        color: Option<Color>,
        radius: Option<Radius>,
        label: Option<Text>,
        scale: Option<HalfSizes3D>,
        rotation: Option<Rotation3D>,
    }

    // TODO(cmc): This could be optimized by using custom property accessors.
    impl Vertex {
        fn from_props(
            mut props: LinkedHashMap<String, Property>,
            ignored_props: &mut BTreeSet<String>,
        ) -> Option<Vertex> {
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

            // Gaussian splatting using models from e.g. https://poly.cam/tools/gaussian-splatting
            const PROPS_SCALE_X: &str = "scale_0";
            const PROPS_SCALE_Y: &str = "scale_1";
            const PROPS_SCALE_Z: &str = "scale_2";
            const PROPS_QUAT_W: &str = "rot_0";
            const PROPS_QUAT_X: &str = "rot_1";
            const PROPS_QUAT_Y: &str = "rot_2";
            const PROPS_QUAT_Z: &str = "rot_3";
            const PROPS_SH_DC_0: &str = "f_dc_0";
            const PROPS_SH_DC_1: &str = "f_dc_1";
            const PROPS_SH_DC_2: &str = "f_dc_2";
            const PROP_OPACITY: &str = "opacity";

            let (Some(x), Some(y), Some(z)) = (
                props.get(PROP_X).and_then(f32),
                props.get(PROP_Y).and_then(f32),
                props.get(PROP_Z).and_then(f32),
            ) else {
                // All points much have positions.
                for (key, _value) in props {
                    ignored_props.insert(key);
                }
                return None;
            };

            // We remove properties as they are read so we can warn about the ones we don't recognize.
            props.remove(PROP_X);
            props.remove(PROP_Y);
            props.remove(PROP_Z);

            let mut this = Self {
                position: Position3D::new(x, y, z),
                color: None,
                radius: None,
                label: None,
                scale: None,
                rotation: None,
            };

            if let (Some(r), Some(g), Some(b)) = (
                props.get(PROP_RED).and_then(u8),
                props.get(PROP_GREEN).and_then(u8),
                props.get(PROP_BLUE).and_then(u8),
            ) {
                let a = props.get(PROP_ALPHA).and_then(u8).unwrap_or(255);

                props.remove(PROP_RED);
                props.remove(PROP_GREEN);
                props.remove(PROP_BLUE);
                props.remove(PROP_ALPHA);

                this.color = Some(Color::new((r, g, b, a)));
            };

            if let (Some(r_dc), Some(g_dc), Some(b_dc)) = (
                props.get(PROPS_SH_DC_0).and_then(f32),
                props.get(PROPS_SH_DC_1).and_then(f32),
                props.get(PROPS_SH_DC_2).and_then(f32),
            ) {
                fn to_u8(f: f32) -> u8 {
                    (f * 255.0 + 0.5) as u8
                }

                // See http://en.wikipedia.org/wiki/Table_of_spherical_harmonics
                let sp_c0 = 0.5 * (1.0 / std::f32::consts::PI).sqrt();

                // Evaluate the zero-degree Spherical Harmonic to get the ambient RGB:
                let r = to_u8(0.5 + sp_c0 * r_dc);
                let g = to_u8(0.5 + sp_c0 * g_dc);
                let b = to_u8(0.5 + sp_c0 * b_dc);

                // Convert opacity to alpha (if any):
                let a = props
                    .get(PROP_OPACITY)
                    .and_then(f32)
                    .map(|opacity| 1.0 / (1.0 + (-opacity).exp()));
                let a = a.map_or(255, to_u8);

                props.remove(PROPS_SH_DC_0);
                props.remove(PROPS_SH_DC_1);
                props.remove(PROPS_SH_DC_2);
                props.remove(PROP_OPACITY);
                this.color = Some(Color::new((r, g, b, a)));
            }

            if let (Some(x), Some(y), Some(z)) = (
                props.get(PROPS_SCALE_X).and_then(f32),
                props.get(PROPS_SCALE_Y).and_then(f32),
                props.get(PROPS_SCALE_Z).and_then(f32),
            ) {
                props.remove(PROPS_SCALE_X);
                props.remove(PROPS_SCALE_Y);
                props.remove(PROPS_SCALE_Z);
                this.scale = Some(HalfSizes3D::new(x.exp(), y.exp(), z.exp())); // Gaussian splatting files store the log scale.
            }

            if let (Some(x), Some(y), Some(z), Some(w)) = (
                props.get(PROPS_QUAT_X).and_then(f32),
                props.get(PROPS_QUAT_Y).and_then(f32),
                props.get(PROPS_QUAT_Z).and_then(f32),
                props.get(PROPS_QUAT_W).and_then(f32),
            ) {
                props.remove(PROPS_QUAT_X);
                props.remove(PROPS_QUAT_Y);
                props.remove(PROPS_QUAT_Z);
                props.remove(PROPS_QUAT_W);
                this.rotation = Some(Rotation3D::from(glam::Quat { x, y, z, w }.normalize()));
            }

            // rot_0…, scale_0, …

            if let Some(radius) = props.get(PROP_RADIUS).and_then(f32) {
                props.remove(PROP_RADIUS);
                this.radius = Some(Radius(radius));
            }

            if let Some(label) = props.get(PROP_LABEL).and_then(string) {
                this.label = Some(Text(label.to_string().into()));
                props.remove(PROP_LABEL);
            }

            for (key, _value) in props {
                if key.starts_with("f_rest_") {
                    // f_rest_0, f_rest_1, f_rest_2, …
                    ignored_props.insert("f_rest_*".to_owned());
                } else {
                    ignored_props.insert(key);
                }
            }

            Some(this)
        }
    }

    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut radii = Vec::new();
    let mut scales = Vec::new();
    let mut rotations = Vec::new();
    let mut labels = Vec::new();

    let mut ignored_props = BTreeSet::new();

    for (key, all_props) in ply.payload {
        if key == "vertex" {
            for props in all_props {
                if let Some(vertex) = Vertex::from_props(props, &mut ignored_props) {
                    let Vertex {
                        position,
                        color,
                        radius,
                        label,
                        scale,
                        rotation,
                    } = vertex;
                    positions.push(position);
                    colors.push(color); // opt
                    radii.push(radius); // opt
                    scales.push(scale); // opt
                    rotations.push(rotation); // opt
                    labels.push(label); // opt
                }
            }
        } else {
            re_log::warn!("Ignoring {key:?} in .ply file");
        }
    }

    if !ignored_props.is_empty() {
        re_log::warn!("Ignored properties of .ply file: {ignored_props:?}");
    }

    re_tracing::profile_scope!("fill-in");

    let mut arch = crate::archetypes::Points3D::new(positions);
    if colors.iter().any(|opt| opt.is_some()) {
        // If some colors have been specified but not others, default the unspecified ones to white.
        let colors = colors
            .into_iter()
            .map(|opt| opt.unwrap_or(Color::from_rgb(255, 255, 255)));
        arch = arch.with_colors(colors);
    }
    if radii.iter().any(|opt| opt.is_some()) {
        // If some radii have been specified but not others, default the unspecified ones to 1.0.
        let radii = radii.into_iter().map(|opt| opt.unwrap_or(Radius(1.0)));
        arch = arch.with_radii(radii);
    }
    if scales.iter().any(|opt| opt.is_some()) {
        let scales = scales
            .into_iter()
            .map(|opt| opt.unwrap_or(HalfSizes3D::new(1.0, 1.0, 1.0)));
        arch = arch.with_scales(scales);
    }
    if rotations.iter().any(|opt| opt.is_some()) {
        let rotations = rotations
            .into_iter()
            .map(|opt| opt.unwrap_or(Rotation3D::from(Quaternion::IDENTITY)));
        arch = arch.with_rotations(rotations);
    }
    if labels.iter().any(|opt| opt.is_some()) {
        // If some labels have been specified but not others, default the unspecified ones to "undef".
        let labels = labels
            .into_iter()
            .map(|opt| opt.unwrap_or(Text("undef".into())));
        arch = arch.with_labels(labels);
    }

    arch
}
