//! Types used for the log data.

#![allow(clippy::manual_range_contains)]

use std::{collections::BTreeMap, fmt::Write as _, ops::RangeInclusive};

pub mod encoding;

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

/// A date-time represented as nanoseconds since unix epoch
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Time(i64);

impl Time {
    // #[inline]
    // pub fn now() -> Self {
    //     Self(nanos_since_epoch())
    // }

    #[inline]
    pub fn nanos_since_epoch(&self) -> i64 {
        self.0
    }

    pub fn from_ns_since_epoch(ns_since_epoch: i64) -> Self {
        Self(ns_since_epoch)
    }

    pub fn from_us_since_epoch(us_since_epoch: i64) -> Self {
        Self(us_since_epoch * 1_000)
    }

    /// Human-readable formatting
    pub fn format(&self) -> String {
        let nanos_since_epoch = self.nanos_since_epoch();
        let years_since_epoch = nanos_since_epoch / 1_000_000_000 / 60 / 60 / 24 / 365;

        if 50 <= years_since_epoch && years_since_epoch <= 150 {
            use chrono::TimeZone as _;
            let datetime = chrono::Utc.timestamp(
                nanos_since_epoch / 1_000_000_000,
                (nanos_since_epoch % 1_000_000_000) as _,
            );

            if datetime.date() == chrono::offset::Utc::today() {
                datetime.format("%H:%M:%S%.6fZ").to_string()
            } else {
                datetime.format("%Y-%m-%d %H:%M:%S%.6fZ").to_string()
            }
        } else {
            let secs = nanos_since_epoch as f64 * 1e-9;
            // assume relative time
            format!("+{:.03}s", secs)
        }
    }

    pub fn lerp(range: RangeInclusive<Time>, t: f32) -> Time {
        let (min, max) = (range.start().0, range.end().0);
        Self(min + ((max - min) as f64 * (t as f64)).round() as i64)
    }
}

impl std::fmt::Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format().fmt(f)
    }
}

impl std::ops::Sub for Time {
    type Output = Duration;
    fn sub(self, rhs: Time) -> Duration {
        Duration(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::AddAssign<Duration> for Time {
    fn add_assign(&mut self, duration: Duration) {
        self.0 = self.0.saturating_add(duration.0);
    }
}

impl TryFrom<std::time::SystemTime> for Time {
    type Error = std::time::SystemTimeError;

    fn try_from(time: std::time::SystemTime) -> Result<Time, Self::Error> {
        time.duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Time(duration_since_epoch.as_nanos() as _))
    }
}

// ----------------------------------------------------------------------------

/// A signed duration represented as nanoseconds since unix epoch
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Duration(i64);

impl Duration {
    pub fn from_secs(secs: f32) -> Self {
        Self((secs * 1e9).round() as _)
    }

    pub fn as_secs_f32(&self) -> f32 {
        self.0 as f32 * 1e-9
    }
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
    Sequence(u64),
}

impl TimeValue {
    /// Where in the range is this value? Returns 0-1 if within the range.
    /// Returns <0 if before, >1 if after, and `None` if the unit is wrong.
    pub fn lerp_t(&self, range: RangeInclusive<TimeValue>) -> Option<f32> {
        fn lerp_t_i64(min: i64, value: i64, max: i64) -> f32 {
            if min == max {
                0.5
            } else {
                value.saturating_sub(min) as f32 / max.saturating_sub(min) as f32
            }
        }

        match (range.start(), *self, range.end()) {
            (TimeValue::Time(min), TimeValue::Time(value), TimeValue::Time(max)) => {
                Some(lerp_t_i64(
                    min.nanos_since_epoch(),
                    value.nanos_since_epoch(),
                    max.nanos_since_epoch(),
                ))
            }
            (TimeValue::Sequence(min), TimeValue::Sequence(value), TimeValue::Sequence(max)) => {
                Some(lerp_t_i64(*min as _, value as _, *max as _))
            }
            _ => None,
        }
    }

    pub fn lerp(range: RangeInclusive<TimeValue>, t: f32) -> Option<TimeValue> {
        fn lerp_i64(range: RangeInclusive<i64>, t: f32) -> i64 {
            let (min, max) = (*range.start(), *range.end());
            min + ((max - min) as f64 * (t as f64)).round() as i64
        }

        match (*range.start(), *range.end()) {
            (TimeValue::Time(min), TimeValue::Time(max)) => {
                Some(TimeValue::Time(Time::lerp(min..=max, t)))
            }
            (TimeValue::Sequence(min), TimeValue::Sequence(max)) => {
                Some(TimeValue::Sequence(lerp_i64(min as _..=max as _, t) as _))
            }
            _ => None,
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
impl_into_enum!(u64, TimeValue, Sequence);

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
            Self::Pos3(_) | Self::LineSegments3D(_) | Self::Mesh3D(_) => true,
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
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Image {
    // TODO: pub pos: [f32; 2],
    pub size: [u32; 2],
    pub format: ImageFormat,
    pub data: Vec<u8>,
}
