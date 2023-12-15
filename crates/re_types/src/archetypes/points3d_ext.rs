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
        use anyhow::Context as _;

        let file = std::fs::File::open(filepath)
            .with_context(|| format!("Failed to open file {filepath:?}"))?;
        let mut file = std::io::BufReader::new(file);

        let parser = ply_rs::parser::Parser::<ply_rs::ply::DefaultElement>::new();
        let ply = parser.read_ply(&mut file)?;

        Ok(from_ply(&ply))
    }

    /// Creates a new [`Points3D`] from the contents of a `.ply` file.
    ///
    /// If unspecified, he media type will be inferred from the contents.
    pub fn from_file_contents(contents: &[u8]) -> anyhow::Result<Self> {
        let parser = ply_rs::parser::Parser::<ply_rs::ply::DefaultElement>::new();
        let mut contents = std::io::Cursor::new(contents);
        let ply = parser.read_ply(&mut contents)?;
        Ok(from_ply(&ply))
    }
}

fn from_ply(ply: &ply_rs::ply::Ply<ply_rs::ply::DefaultElement>) -> Points3D {
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
    }

    // TODO(cmc): This could be optimized by using custom property accessors.
    impl Vertex {
        fn from_props(props: &LinkedHashMap<String, Property>) -> Option<Vertex> {
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

            let (Some(x), Some(y), Some(z)) = (
                props.get(PROP_X).and_then(f32),
                props.get(PROP_Y).and_then(f32),
                props.get(PROP_Z).and_then(f32),
            ) else {
                return None;
            };

            let mut this = Self {
                position: Position3D::new(x, y, z),
                color: None,
                radius: None,
                label: None,
            };

            if let (Some(r), Some(g), Some(b)) = (
                props.get(PROP_RED).and_then(u8),
                props.get(PROP_GREEN).and_then(u8),
                props.get(PROP_BLUE).and_then(u8),
            ) {
                let a = props.get(PROP_ALPHA).and_then(u8).unwrap_or(255);
                this.color = Some(Color::new((r, g, b, a)));
            };

            if let Some(radius) = props.get(PROP_RADIUS).and_then(f32) {
                this.radius = Some(Radius(radius));
            }

            if let Some(label) = props.get(PROP_LABEL).and_then(string) {
                this.label = Some(Text(label.to_string().into()));
            }

            Some(this)
        }
    }

    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut radii = Vec::new();
    let mut labels = Vec::new();

    for all_props in ply.payload.values() {
        for props in all_props {
            if let Some(vertex) = Vertex::from_props(props) {
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
    }

    colors.truncate(positions.len());
    radii.truncate(positions.len());
    labels.truncate(positions.len());

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
    if labels.iter().any(|opt| opt.is_some()) {
        // If some labels have been specified but not others, default the unspecified ones to "undef".
        let labels = labels
            .into_iter()
            .map(|opt| opt.unwrap_or(Text("undef".into())));
        arch = arch.with_labels(labels);
    }

    arch
}
