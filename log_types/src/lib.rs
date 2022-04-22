//! Types used for the log data.

#![allow(clippy::manual_range_contains)]

#[cfg(any(feature = "save", feature = "load"))]
pub mod encoding;

mod time;

pub use time::{Duration, Time};

use std::{collections::BTreeMap, fmt::Write as _};

macro_rules! impl_into_enum {
    ($from_ty: ty, $enum_name: ident, $to_enum_variant: ident) => {
        impl From<$from_ty> for $enum_name {
            fn from(value: $from_ty) -> Self {
                Self::$to_enum_variant(value)
            }
        }
    };
}

// ----------------------------------------------------------------------------

/// A unique id per [`LogMsg`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct LogId(uuid::Uuid);

impl nohash_hasher::IsEnabled for LogId {}

// required for nohash_hasher
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for LogId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl LogId {
    fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct LogMsg {
    /// A unique id per [`LogMsg`].
    pub id: LogId,

    /// Time information (when it was logged, when it was received, …)
    pub time_point: TimePoint,

    /// What this is.
    pub object_path: ObjectPath,

    /// The value of this.
    pub data: Data,

    /// Where ("camera", "world") this thing is in.
    pub space: Option<ObjectPath>,
}

impl LogMsg {
    pub fn space(mut self, space: &ObjectPath) -> Self {
        self.space = Some(space.clone());
        self
    }
}

pub fn log_msg(time_point: &TimePoint, object_path: ObjectPath, data: impl Into<Data>) -> LogMsg {
    LogMsg {
        time_point: time_point.clone(),
        object_path,
        data: data.into(),
        space: None,
        id: LogId::random(),
    }
}

// ----------------------------------------------------------------------------

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(pub BTreeMap<String, TimeValue>);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeValue {
    Time(Time),
    Sequence(i64),
}

impl TimeValue {
    /// Offset by arbitrary value.
    /// Nanos for time.
    #[must_use]
    pub fn add_offset_f32(self, offset: f32) -> Self {
        self.add_offset_f64(offset as f64)
    }

    /// Offset by arbitrary value.
    /// Nanos for time.
    #[must_use]
    pub fn add_offset_f64(self, offset: f64) -> Self {
        match self {
            Self::Time(time) => Self::Time(time + Duration::from_nanos(offset as _)),
            Self::Sequence(seq) => Self::Sequence(seq.saturating_add(offset as _)),
        }
    }
}

impl std::fmt::Display for TimeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Time(time) => time.format().fmt(f),
            Self::Sequence(seq) => format!("#{seq}").fmt(f),
        }
    }
}

impl_into_enum!(Time, TimeValue, Time);
impl_into_enum!(i64, TimeValue, Sequence);

pub fn time_point(fields: impl IntoIterator<Item = (&'static str, TimeValue)>) -> TimePoint {
    TimePoint(
        fields
            .into_iter()
            .map(|(name, tt)| (name.to_string(), tt))
            .collect(),
    )
}

// ----------------------------------------------------------------------------

/// A hierarchy
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ObjectPath(pub Vec<ObjectPathComponent>);

impl ObjectPath {
    pub fn sibling(&self, last_comp: impl Into<ObjectPathComponent>) -> Self {
        let mut path = self.0.clone();
        path.pop(); // TODO: handle root?
        path.push(last_comp.into());
        Self(path)
    }
}

impl std::fmt::Display for ObjectPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char('/')?;
        for (i, comp) in self.0.iter().enumerate() {
            comp.fmt(f)?;
            if i + 1 != self.0.len() {
                f.write_char('/')?;
            }
        }
        Ok(())
    }
}

impl From<ObjectPathComponent> for ObjectPath {
    fn from(component: ObjectPathComponent) -> Self {
        Self(vec![component])
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjectPathComponent {
    /// Named child
    String(String),
    /// Many children with identities that persist over time
    PersistId(String, Identifier),
    /// Many children with transient identities that only are valid for a single [`TimePoint`].
    TempId(String, Identifier),
}

impl std::fmt::Display for ObjectPathComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(string) => f.write_str(string),
            // Self::PersistId(string, id) => f.write_str(&format!("Persist({string}={id}))),
            // Self::TempId(string, id) => f.write_str(&format!("Temp({string}={id}))),
            Self::PersistId(string, id) => f.write_str(&format!("{string}={id}")),
            Self::TempId(string, id) => f.write_str(&format!("{string}=~{id}")),
        }
    }
}

impl From<&str> for ObjectPathComponent {
    fn from(comp: &str) -> Self {
        Self::String(comp.to_owned())
    }
}

pub fn persist_id(name: impl Into<String>, id: impl Into<Identifier>) -> ObjectPathComponent {
    ObjectPathComponent::PersistId(name.into(), id.into())
}

pub fn temp_id(name: impl Into<String>, id: impl Into<Identifier>) -> ObjectPathComponent {
    ObjectPathComponent::TempId(name.into(), id.into())
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Identifier {
    String(String),
    U64(u64),
    Sequence(u64),
    // Uuid, …
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(value) => value.fmt(f),
            Self::U64(value) => value.fmt(f),
            Self::Sequence(seq) => format!("#{seq}").fmt(f),
        }
    }
}

impl_into_enum!(String, Identifier, String);
impl_into_enum!(u64, Identifier, U64);

impl std::ops::Div for ObjectPathComponent {
    type Output = ObjectPath;
    fn div(self, rhs: ObjectPathComponent) -> Self::Output {
        ObjectPath(vec![self, rhs])
    }
}

impl std::ops::Div<ObjectPathComponent> for ObjectPath {
    type Output = ObjectPath;
    fn div(mut self, rhs: ObjectPathComponent) -> Self::Output {
        self.0.push(rhs);
        self
    }
}

impl std::ops::Div<ObjectPathComponent> for &ObjectPath {
    type Output = ObjectPath;
    fn div(self, rhs: ObjectPathComponent) -> Self::Output {
        self.clone() / rhs
    }
}

impl std::ops::Div<&'static str> for ObjectPath {
    type Output = ObjectPath;
    fn div(mut self, rhs: &'static str) -> Self::Output {
        self.0.push(ObjectPathComponent::String(rhs.into()));
        self
    }
}

impl std::ops::Div<&'static str> for &ObjectPath {
    type Output = ObjectPath;
    fn div(self, rhs: &'static str) -> Self::Output {
        self.clone() / rhs
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Data {
    // 1D:
    I32(i32),
    F32(f32),

    /// RGBA unmultiplied/separate alpha
    Color([u8; 4]),

    // ----------------------------
    // 2D:
    /// Special sibling attributes: "color", "radius"
    Pos2([f32; 2]),
    /// Special sibling attributes: "color"
    BBox2D(BBox2D),
    Image(Image),

    // ----------------------------
    // 3D:
    /// Special sibling attributes: "color", "radius"
    Pos3([f32; 3]),
    Path3D(Vec<[f32; 3]>),
    /// Special sibling attributes: "color", "radius"
    LineSegments3D(Vec<[[f32; 3]; 2]>),
    Mesh3D(Mesh3D),

    // ----------------------------
    // N-D:
    Vecf32(Vec<f32>),
}

impl Data {
    pub fn is_2d(&self) -> bool {
        match self {
            Self::I32(_)
            | Self::F32(_)
            | Self::Color(_)
            | Self::Pos3(_)
            | Self::Path3D(_)
            | Self::LineSegments3D(_)
            | Self::Mesh3D(_)
            | Self::Vecf32(_) => false,
            Self::Pos2(_) | Self::BBox2D(_) | Self::Image(_) => true,
        }
    }

    pub fn is_3d(&self) -> bool {
        match self {
            Self::I32(_)
            | Self::F32(_)
            | Self::Color(_)
            | Self::Pos2(_)
            | Self::BBox2D(_)
            | Self::Image(_)
            | Self::Vecf32(_) => false,
            Self::Pos3(_) | Self::Path3D(_) | Self::LineSegments3D(_) | Self::Mesh3D(_) => true,
        }
    }
}

impl_into_enum!(i32, Data, I32);
impl_into_enum!(f32, Data, F32);
impl_into_enum!(BBox2D, Data, BBox2D);
impl_into_enum!(Vec<f32>, Data, Vecf32);
impl_into_enum!(Image, Data, Image);
impl_into_enum!(Mesh3D, Data, Mesh3D);

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BBox2D {
    /// Upper left corner.
    pub min: [f32; 2],
    /// Lower right corner.
    pub max: [f32; 2],
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum MeshFormat {
    Gltf,
    Glb,
    Obj,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Mesh3D {
    pub format: MeshFormat,
    pub bytes: std::sync::Arc<[u8]>,
    /// four columns of a transformation matrix
    pub transform: [[f32; 4]; 4],
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ImageFormat {
    Luminance8,
    Rgba8,
    Jpeg,
}

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Image {
    // TODO: pub pos: [f32; 2],
    /// Must always be set and correct, even for [`ImageFormat::Jpeg`].
    pub size: [u32; 2],
    pub format: ImageFormat,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("size", &self.size)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}
