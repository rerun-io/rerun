//! Types used for the log data.

#![allow(clippy::manual_range_contains)]

#[cfg(any(feature = "save", feature = "load"))]
pub mod encoding;

mod data;
mod path;
mod time;

pub use data::*;
pub use path::*;
pub use time::{Duration, Time};

use std::collections::BTreeMap;

#[macro_export]
macro_rules! impl_into_enum {
    ($from_ty: ty, $enum_name: ident, $to_enum_variant: ident) => {
        impl From<$from_ty> for $enum_name {
            #[inline]
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

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for LogId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl LogId {
    #[inline]
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum LogMsg {
    /// Log type-into to a [`TypePath`].
    TypeMsg(TypeMsg),
    /// Log some data to a [`DataPath`].
    DataMsg(DataMsg),
}

impl LogMsg {
    pub fn id(&self) -> LogId {
        match self {
            Self::TypeMsg(msg) => msg.id,
            Self::DataMsg(msg) => msg.id,
        }
    }
}

impl_into_enum!(TypeMsg, LogMsg, TypeMsg);
impl_into_enum!(DataMsg, LogMsg, DataMsg);

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TypeMsg {
    /// A unique id per [`LogMsg`].
    pub id: LogId,

    /// The [`TypePath`] target.
    pub type_path: TypePath,

    /// The type of object at this object type path.
    pub object_type: ObjectType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjectType {
    Image,
    Point2D,
    BBox2d,
    LineSegments2D,

    Point3D,
    Box3D,
    Path3D,
    LineSegments3D,
    Mesh3D,
    Camera,
}

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataMsg {
    /// A unique id per [`DataMsg`].
    pub id: LogId,

    /// Time information (when it was logged, when it was received, …)
    pub time_point: TimePoint,

    /// What this is.
    pub data_path: DataPath,

    /// The value of this.
    pub data: Data,

    /// Where ("camera", "world") this thing is in.
    pub space: Option<DataPath>,
}

impl DataMsg {
    #[inline]
    pub fn space(mut self, space: &DataPath) -> Self {
        self.space = Some(space.clone());
        self
    }
}

#[inline]
pub fn data_msg(time_point: &TimePoint, data_path: DataPath, data: impl Into<Data>) -> DataMsg {
    DataMsg {
        time_point: time_point.clone(),
        data_path,
        data: data.into(),
        space: None,
        id: LogId::random(),
    }
}

// ----------------------------------------------------------------------------

rr_string_interner::declare_new_type!(
    /// The name of a time source. Often something like `"time"` or `"frame"`.
    pub struct TimeSource;
);

impl Default for TimeSource {
    fn default() -> Self {
        Self::new("")
    }
}

// ----------------------------------------------------------------------------

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(pub BTreeMap<TimeSource, TimeValue>);

/// The type of a [`TimeValue`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum TimeType {
    /// Normal wall time.
    Time,

    /// Used e.g. for frames in a film.
    Sequence,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeValue {
    /// Normal wall time.
    Time(Time),

    /// Used e.g. for frames in a film.
    Sequence(i64),
}

impl TimeValue {
    /// Offset by arbitrary value.
    /// Nanos for time.
    #[must_use]
    #[inline]
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

    pub fn typ(&self) -> TimeType {
        match self {
            Self::Time(_) => TimeType::Time,
            Self::Sequence(_) => TimeType::Sequence,
        }
    }

    /// Either nanos since epoch, or a sequence number.
    pub fn as_i64(&self) -> i64 {
        match self {
            Self::Time(time) => time.nanos_since_epoch(),
            Self::Sequence(seq) => *seq,
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

#[inline]
pub fn time_point(fields: impl IntoIterator<Item = (&'static str, TimeValue)>) -> TimePoint {
    TimePoint(
        fields
            .into_iter()
            .map(|(name, tt)| (TimeSource::from(name), tt))
            .collect(),
    )
}
