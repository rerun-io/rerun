//! Types used for the log data.

#![allow(clippy::manual_range_contains)]

#[cfg(any(feature = "save", feature = "load"))]
pub mod encoding;

mod data;
mod time;

pub use data::*;
pub use time::{Duration, Time};

use std::{collections::BTreeMap, fmt::Write as _};

use rr_string_interner::InternedString;

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

// required for nohash_hasher
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for LogId {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.as_u128() as u64);
    }
}

impl LogId {
    #[inline]
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
    pub data_path: DataPath,

    /// The value of this.
    pub data: Data,

    /// Where ("camera", "world") this thing is in.
    pub space: Option<DataPath>,
}

impl LogMsg {
    #[inline]
    pub fn space(mut self, space: &DataPath) -> Self {
        self.space = Some(space.clone());
        self
    }
}

#[inline]
pub fn log_msg(time_point: &TimePoint, data_path: DataPath, data: impl Into<Data>) -> LogMsg {
    LogMsg {
        time_point: time_point.clone(),
        data_path,
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
            .map(|(name, tt)| (name.to_string(), tt))
            .collect(),
    )
}

// ----------------------------------------------------------------------------

/// A path to a specific piece of data (e.g. a single `f32`).
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataPath(pub Vec<DataPathComponent>);

impl DataPath {
    #[inline]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn parent(&self) -> Self {
        let mut path = self.0.clone();
        path.pop();
        Self(path)
    }

    pub fn sibling(&self, last_comp: impl Into<DataPathComponent>) -> Self {
        let mut path = self.0.clone();
        path.pop(); // TODO: handle root?
        path.push(last_comp.into());
        Self(path)
    }
}

impl std::fmt::Display for DataPath {
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

impl From<&str> for DataPath {
    #[inline]
    fn from(component: &str) -> Self {
        Self(vec![component.into()])
    }
}

impl From<DataPathComponent> for DataPath {
    #[inline]
    fn from(component: DataPathComponent) -> Self {
        Self(vec![component])
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DataPathComponent {
    /// Struct member. Each member can have a different type.
    String(InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(Index),
}

impl std::fmt::Display for DataPathComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(string) => f.write_str(string),
            Self::Index(index) => index.fmt(f),
        }
    }
}

impl From<&str> for DataPathComponent {
    #[inline]
    fn from(comp: &str) -> Self {
        Self::String(comp.into())
    }
}

/// The key of a table.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Index {
    /// For arrays, assumed to be dense (0, 1, 2, …).
    Sequence(u64),

    /// X,Y pixel coordinates, from top left.
    Pixel([u64; 2]),

    /// Any integer, e.g. a hash or an arbitrary identifier.
    Integer(i128),

    /// UUID/GUID
    Uuid(uuid::Uuid),

    /// Anything goes.
    String(String),
}

impl std::fmt::Display for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequence(seq) => format!("#{seq}").fmt(f),
            Self::Pixel([x, y]) => format!("[{x}, {y}]").fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Uuid(value) => value.fmt(f),
            Self::String(value) => format!("{value:?}").fmt(f), // put it in quotes
        }
    }
}

impl_into_enum!(String, Index, String);

impl std::ops::Div for DataPathComponent {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: DataPathComponent) -> Self::Output {
        DataPath(vec![self, rhs])
    }
}

impl std::ops::Div<DataPathComponent> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: DataPathComponent) -> Self::Output {
        self.0.push(rhs);
        self
    }
}

impl std::ops::Div<Index> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: Index) -> Self::Output {
        self.0.push(DataPathComponent::Index(rhs));
        self
    }
}

impl std::ops::Div<Index> for &DataPath {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: Index) -> Self::Output {
        self.clone() / rhs
    }
}

impl std::ops::Div<DataPathComponent> for &DataPath {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: DataPathComponent) -> Self::Output {
        self.clone() / rhs
    }
}

impl std::ops::Div<&'static str> for DataPath {
    type Output = DataPath;

    #[inline]
    fn div(mut self, rhs: &'static str) -> Self::Output {
        self.0.push(DataPathComponent::String(rhs.into()));
        self
    }
}

impl std::ops::Div<&'static str> for &DataPath {
    type Output = DataPath;

    #[inline]
    fn div(self, rhs: &'static str) -> Self::Output {
        self.clone() / rhs
    }
}
