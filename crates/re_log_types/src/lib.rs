//! Types used for the log data.

#![allow(clippy::manual_range_contains)]

#[cfg(any(feature = "save", feature = "load"))]
pub mod encoding;

mod data;
pub mod hash;
mod index;
pub mod objects;
mod path;
mod time;

pub use data::*;
pub use index::*;
pub use objects::ObjectType;
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
pub struct MsgId(uuid::Uuid);

impl nohash_hasher::IsEnabled for MsgId {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for MsgId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl MsgId {
    #[inline]
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[allow(clippy::large_enum_variant)]
pub enum LogMsg {
    /// Log type-into to a [`ObjTypePath`].
    TypeMsg(TypeMsg),
    /// Log some data to a [`DataPath`].
    DataMsg(DataMsg),
}

impl LogMsg {
    pub fn id(&self) -> MsgId {
        match self {
            Self::TypeMsg(msg) => msg.msg_id,
            Self::DataMsg(msg) => msg.msg_id,
        }
    }
}

impl_into_enum!(TypeMsg, LogMsg, TypeMsg);
impl_into_enum!(DataMsg, LogMsg, DataMsg);

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TypeMsg {
    /// A unique id per [`LogMsg`].
    pub msg_id: MsgId,

    /// The [`ObjTypePath`] target.
    pub type_path: ObjTypePath,

    /// The type of object at this object type path.
    pub object_type: ObjectType,
}

impl TypeMsg {
    pub fn object_type(type_path: ObjTypePath, object_type: ObjectType) -> Self {
        Self {
            msg_id: MsgId::random(),
            type_path,
            object_type,
        }
    }
}

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataMsg {
    /// A unique id per [`DataMsg`].
    pub msg_id: MsgId,

    /// Time information (when it was logged, when it was received, …)
    pub time_point: TimePoint,

    /// What the data is targeting.
    pub data_path: DataPath,

    /// The value of this.
    pub data: LoggedData,
}

#[inline]
pub fn data_msg(
    time_point: &TimePoint,
    obj_path: impl Into<ObjPath>,
    field_name: impl Into<FieldName>,
    data: impl Into<LoggedData>,
) -> DataMsg {
    DataMsg {
        time_point: time_point.clone(),
        data_path: DataPath::new(obj_path.into(), field_name.into()),
        data: data.into(),
        msg_id: MsgId::random(),
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum LoggedData {
    /// A single data value
    Single(Data),

    /// Log multiple values at once.
    ///
    /// The index replaces the last index in [`DataMsg.data_path`], which should be [`Index::Placeholder]`.
    Batch { indices: Vec<Index>, data: DataVec },

    /// Log the same value for all objects sharing the same index prefix (i.e. ignoring the last index).
    ///
    /// The last index in [`DataMsg.data_path`] should be [`Index::Placeholder]`.
    ///
    /// You can only use this for optional fields such as `color`, `space` etc.
    /// You can NOT use it for primary fields such as `pos`.
    BatchSplat(Data),
}

impl LoggedData {
    #[inline]
    pub fn data_type(&self) -> DataType {
        match self {
            Self::Single(data) | Self::BatchSplat(data) => data.data_type(),
            Self::Batch { data, .. } => data.data_type(),
        }
    }
}

impl From<Data> for LoggedData {
    #[inline]
    fn from(data: Data) -> Self {
        Self::Single(data)
    }
}

#[macro_export]
macro_rules! impl_into_logged_data {
    ($from_ty: ty, $data_enum_variant: ident) => {
        impl From<$from_ty> for LoggedData {
            #[inline]
            fn from(value: $from_ty) -> Self {
                Self::Single(Data::$data_enum_variant(value))
            }
        }
    };
}

impl_into_logged_data!(i32, I32);
impl_into_logged_data!(f32, F32);
impl_into_logged_data!(BBox2D, BBox2D);
impl_into_logged_data!(Tensor, Tensor);
impl_into_logged_data!(Box3, Box3);
impl_into_logged_data!(Mesh3D, Mesh3D);
impl_into_logged_data!(Camera, Camera);
impl_into_logged_data!(ObjPath, Space);

// ----------------------------------------------------------------------------

re_string_interner::declare_new_type!(
    /// The name of a time source. Often something like `"log_time"` or `"frame_nr"`.
    pub struct TimeSourceName;
);

impl Default for TimeSourceName {
    fn default() -> Self {
        Self::new("")
    }
}

// ----------------------------------------------------------------------------

/// A time frame/space, e.g. `log_time` or `frame_nr`, coupled with the type of time
/// it keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeSource {
    /// Name of the time source (e.g. "log_time").
    name: TimeSourceName,

    /// Sequence or time?
    typ: TimeType,
}

impl Default for TimeSource {
    fn default() -> Self {
        Self {
            name: TimeSourceName::new(""),
            typ: TimeType::Sequence,
        }
    }
}

impl TimeSource {
    #[inline]
    pub fn new(name: impl Into<TimeSourceName>, typ: TimeType) -> Self {
        Self {
            name: name.into(),
            typ,
        }
    }

    #[inline]
    pub fn name(&self) -> &TimeSourceName {
        &self.name
    }

    #[inline]
    pub fn typ(&self) -> TimeType {
        self.typ
    }
}

impl nohash_hasher::IsEnabled for TimeSource {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for TimeSource {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.name.hash() | self.typ.hash());
    }
}

// ----------------------------------------------------------------------------

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimePoint(pub BTreeMap<TimeSource, TimeValue>);

// ----------------------------------------------------------------------------

/// The type of a [`TimeValue`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeType {
    /// Normal wall time.
    Time,

    /// Used e.g. for frames in a film.
    Sequence,
}

impl TimeType {
    fn hash(&self) -> u64 {
        match self {
            Self::Time => 0,
            Self::Sequence => 1,
        }
    }
}

// ----------------------------------------------------------------------------

/// Either nanoseconds or sequence numbers.
///
/// Must be matched with a [`TimeType`] to know what.
///
/// Used both for time points and durations.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeInt(i64);

impl TimeInt {
    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn as_f32(&self) -> f32 {
        self.0 as _
    }

    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0 as _
    }

    #[inline]
    pub fn abs(&self) -> Self {
        Self(self.0.saturating_abs())
    }
}

impl From<i64> for TimeInt {
    #[inline]
    fn from(seq: i64) -> Self {
        Self(seq)
    }
}

impl From<Duration> for TimeInt {
    #[inline]
    fn from(duration: Duration) -> Self {
        Self(duration.as_nanos())
    }
}

impl From<Time> for TimeInt {
    #[inline]
    fn from(time: Time) -> Self {
        Self(time.nanos_since_epoch())
    }
}

impl From<TimeInt> for Time {
    fn from(int: TimeInt) -> Self {
        Self::from_ns_since_epoch(int.as_i64())
    }
}

impl From<TimeInt> for Duration {
    fn from(int: TimeInt) -> Self {
        Self::from_nanos(int.as_i64())
    }
}

impl std::ops::Neg for TimeInt {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(self.0.saturating_neg())
    }
}

impl std::ops::Add for TimeInt {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for TimeInt {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::AddAssign for TimeInt {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::SubAssign for TimeInt {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl std::iter::Sum for TimeInt {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = TimeInt(0);
        for item in iter {
            sum += item;
        }
        sum
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeValue {
    /// Normal wall time.
    Time(Time),

    /// Used e.g. for frames in a film.
    Sequence(i64),
}

impl TimeValue {
    #[inline]
    pub fn new(typ: TimeType, int: impl Into<TimeInt>) -> Self {
        let int = int.into();
        match typ {
            TimeType::Time => Self::Time(Time::from_ns_since_epoch(int.0)),
            TimeType::Sequence => Self::Sequence(int.0),
        }
    }

    #[inline]
    pub fn time(time: Time) -> Self {
        Self::Time(time)
    }

    #[inline]
    pub fn sequence(seq: i64) -> Self {
        Self::Sequence(seq)
    }

    #[inline]
    pub fn typ(&self) -> TimeType {
        match self {
            Self::Time(_) => TimeType::Time,
            Self::Sequence(_) => TimeType::Sequence,
        }
    }

    /// Either nanos since epoch, or a sequence number.
    #[inline]
    pub fn as_int(&self) -> TimeInt {
        match *self {
            Self::Time(time) => time.into(),
            Self::Sequence(seq) => seq.into(),
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
            .map(|(name, tt)| (TimeSource::new(name, tt.typ()), tt))
            .collect(),
    )
}
