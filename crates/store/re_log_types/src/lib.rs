//! The basic building blocks of the Rerun log format: entity paths, timelines, and store ids.
//!
//! An entity path names *what* was logged and a [`TimePoint`] on one or more timelines names *when*.
//! The messages that carry them between the SDK, an `.rrd` file, and the viewer live in `re_log_msg`,
//! and the data itself is described by `re_sdk_types`.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!
//! ## Mono-components
//!
//! Some components, mostly transform related ones, are "mono-components".
//! This means that Rerun makes assumptions that depend on this component
//! only taking on a singular value for all instances of an Entity. Where possible,
//! exposed APIs will force these components to be logged as a singular instance.
//! However, it is an error with undefined behavior to manually use lower-level
//! APIs to log a batched mono-component.
//!
//! This requirement is especially apparent with transforms:
//! Each entity must have a unique transform chain,
//! e.g. the entity `foo/bar/baz` is has the transform that is the product of
//! `foo.transform * foo/bar.transform * foo/bar/baz.transform`.

mod app_id;
mod entry_id;
mod entry_name;
pub mod example_components;
pub mod hash;
mod index;
pub mod path;
mod store_id;

// mod data_cell;
// mod data_row;
// mod data_table;
mod instance;
mod vec_deque_ext;

use std::sync::Arc;

pub use re_types_core::{InvalidTimelineNameError, TimelineName};

pub use self::app_id::{ApplicationId, InvalidApplicationIdError};
pub use self::entry_id::{EntryId, EntryIdOrName};
pub use self::entry_name::{EntryName, InvalidEntryNameError};
pub use self::index::{
    AbsoluteTimeRange, AbsoluteTimeRangeF, DateVisibility, Duration, IndexWindow, NonMinI64,
    TimeCell, TimeInt, TimePoint, TimeReal, TimeType, Timeline, TimelinePoint, Timestamp,
    TimestampFormat, TimestampFormatKind, TryFromIntError,
};
pub use self::instance::Instance;
pub use self::path::*;
pub use self::store_id::{RecordingId, StoreId, StoreKind};
pub use self::vec_deque_ext::{VecDequeInsertionExt, VecDequeRemovalExt, VecDequeSortingExt};

pub mod external {
    pub use {arrow, re_tuid, re_types_core};
}

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

/// Either the user-chosen name of a table, or an id that is created by the catalog server.
#[derive(
    Debug,
    Clone,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Hash,
    re_byte_size::SizeBytes,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct TableId(Arc<String>);

impl TableId {
    pub fn new(id: String) -> Self {
        Self(Arc::new(id))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for TableId {
    fn from(s: &str) -> Self {
        Self(Arc::new(s.into()))
    }
}

impl From<String> for TableId {
    fn from(s: String) -> Self {
        Self(Arc::new(s))
    }
}

impl std::fmt::Display for TableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// ---

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `log_time` suitable for inserting in a [`TimePoint`].
#[inline]
pub fn build_log_time(log_time: Timestamp) -> (Timeline, TimeInt) {
    (
        Timeline::log_time(),
        TimeInt::new_temporal(log_time.nanos_since_epoch()),
    )
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`TimePoint`].
#[inline]
pub fn build_frame_nr(frame_nr: impl TryInto<TimeInt>) -> (Timeline, TimeInt) {
    (
        Timeline::new("frame_nr", TimeType::Sequence),
        TimeInt::saturated_temporal(frame_nr),
    )
}

#[inline]
pub fn build_index_value(value: impl TryInto<TimeInt>, time_type: TimeType) -> (Timeline, TimeInt) {
    let timeline_name = match time_type {
        TimeType::Sequence => "frame_nr",
        TimeType::DurationNs => "duration",
        TimeType::TimestampNs => "timestamp",
    };

    (
        Timeline::new(timeline_name, time_type),
        TimeInt::saturated_temporal(value),
    )
}
