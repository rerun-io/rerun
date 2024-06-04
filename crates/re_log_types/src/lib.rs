//! The different types that make up the rerun log format.
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

pub mod arrow_msg;
pub mod example_components;
pub mod hash;
pub mod path;
pub mod time_point;

// mod data_cell;
// mod data_row;
// mod data_table;
mod instance;
mod resolved_time_range;
mod time;
mod time_real;
mod vec_deque_ext;

use std::sync::Arc;

use re_build_info::CrateVersion;

pub use self::arrow_msg::{ArrowChunkReleaseCallback, ArrowMsg};
// pub use self::data_cell::{DataCell, DataCellError, DataCellInner, DataCellResult};
// pub use self::data_row::{
//     DataCellRow, DataCellVec, DataReadError, DataReadResult, DataRow, DataRowError, DataRowResult,
//     RowId,
// };
// pub use self::data_table::{
//     DataCellColumn, DataCellOptVec, DataTable, DataTableError, DataTableResult, EntityPathVec,
//     ErasedTimeVec, RowIdVec, TableId, TimePointVec, METADATA_KIND, METADATA_KIND_CONTROL,
//     METADATA_KIND_DATA,
// };
pub use self::instance::Instance;
pub use self::path::*;
pub use self::resolved_time_range::{ResolvedTimeRange, ResolvedTimeRangeF};
pub use self::time::{Duration, Time, TimeZone};
pub use self::time_point::{
    NonMinI64, TimeInt, TimePoint, TimeType, Timeline, TimelineName, TryFromIntError,
};
pub use self::time_real::TimeReal;
pub use self::vec_deque_ext::{VecDequeInsertionExt, VecDequeRemovalExt, VecDequeSortingExt};

pub mod external {
    pub use arrow2;

    pub use re_tuid;
    pub use re_types_core;
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

/// A unique ID for a [`DataRow`].
///
/// ## Semantics
///
/// [`RowId`]s play an important role in how we store, query and garbage collect data.
///
/// ### Storage
///
/// [`RowId`]s must be unique within a `DataStore`. This is enforced by the store's APIs.
///
/// This makes it easy to build and maintain secondary indices around `RowId`s with few to no
/// extraneous state tracking.
///
/// ### Query
///
/// Queries (both latest-at & range semantics) will defer to `RowId` order as a tie-breaker when
/// looking at several rows worth of data that rest at the exact same timestamp.
///
/// In pseudo-code:
/// ```text
/// rr.set_time_sequence("frame", 10)
///
/// rr.log("my_entity", point1, row_id=#1)
/// rr.log("my_entity", point2, row_id=#0)
///
/// rr.query("my_entity", at=("frame", 10))  # returns `point1`
/// ```
///
/// Think carefully about your `RowId`s when logging a lot of data at the same timestamp.
///
/// ### Garbage collection
///
/// Garbage collection happens in `RowId`-order, which roughly means that it happens in the
/// logger's wall-clock order.
///
/// This has very important implications where inserting data far into the past or into the future:
/// think carefully about your `RowId`s in these cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RowId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for RowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl RowId {
    pub const ZERO: Self = Self(re_tuid::Tuid::ZERO);
    pub const MAX: Self = Self(re_tuid::Tuid::MAX);

    /// Create a new unique [`RowId`] based on the current time.
    #[allow(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        Self(re_tuid::Tuid::new())
    }

    /// Returns the next logical [`RowId`].
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`RowId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0.next())
    }

    /// Returns the `n`-next logical [`RowId`].
    ///
    /// This is equivalent to calling [`RowId::next`] `n` times.
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`RowId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn incremented_by(&self, n: u64) -> Self {
        Self(self.0.incremented_by(n))
    }

    /// When the `RowId` was created, in nanoseconds since unix epoch.
    #[inline]
    pub fn nanoseconds_since_epoch(&self) -> u64 {
        self.0.nanoseconds_since_epoch()
    }

    #[inline]
    pub fn from_u128(id: u128) -> Self {
        Self(re_tuid::Tuid::from_u128(id))
    }

    #[inline]
    pub fn as_u128(&self) -> u128 {
        self.0.as_u128()
    }
}

impl re_types_core::SizeBytes for RowId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::ops::Deref for RowId {
    type Target = re_tuid::Tuid;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for RowId {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

re_types_core::delegate_arrow_tuid!(RowId as "rerun.controls.RowId");

// ----------------------------------------------------------------------------

/// What kind of Store this is.
///
/// `Recording` stores contain user-data logged via `log_` API calls.
///
/// In the future, `Blueprint` stores describe how that data is laid out
/// in the viewer, though this is not currently supported.
///
/// Both of these kinds can go over the same stream and be stored in the
/// same datastore, but the viewer wants to treat them very differently.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum StoreKind {
    /// A recording of user-data.
    Recording,

    /// Data associated with the blueprint state.
    Blueprint,
}

impl std::fmt::Display for StoreKind {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Recording => "Recording".fmt(f),
            Self::Blueprint => "Blueprint".fmt(f),
        }
    }
}

/// A unique id per store.
///
/// The kind of store is part of the id, and can be either a
/// [`StoreKind::Recording`] or a [`StoreKind::Blueprint`].
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct StoreId {
    pub kind: StoreKind,
    pub id: Arc<String>,
}

impl StoreId {
    #[inline]
    pub fn random(kind: StoreKind) -> Self {
        Self {
            kind,
            id: Arc::new(uuid::Uuid::new_v4().to_string()),
        }
    }

    #[inline]
    pub fn empty_recording() -> Self {
        Self::from_string(StoreKind::Recording, "<EMPTY>".to_owned())
    }

    #[inline]
    pub fn from_uuid(kind: StoreKind, uuid: uuid::Uuid) -> Self {
        Self {
            kind,
            id: Arc::new(uuid.to_string()),
        }
    }

    #[inline]
    pub fn from_string(kind: StoreKind, str: String) -> Self {
        Self {
            kind,
            id: Arc::new(str),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }

    pub fn is_empty_recording(&self) -> bool {
        self.kind == StoreKind::Recording && self.id.as_str() == "<EMPTY>"
    }
}

impl std::fmt::Display for StoreId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `StoreKind` is not part of how we display the id,
        // because that can easily lead to confusion and bugs
        // when roundtripping to a string (e.g. via Python SDK).
        self.id.fmt(f)
    }
}

// ----------------------------------------------------------------------------

/// The user-chosen name of the application doing the logging.
///
/// Used to categorize recordings.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ApplicationId(pub String);

impl From<&str> for ApplicationId {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for ApplicationId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl ApplicationId {
    /// The default [`ApplicationId`] if the user hasn't set one.
    ///
    /// Currently: `"unknown_app_id"`.
    pub fn unknown() -> Self {
        Self("unknown_app_id".to_owned())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ApplicationId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// ----------------------------------------------------------------------------

/// Command used for activating a blueprint once it has been fully transmitted.
///
/// This command serves two purposes:
/// - It is important that a blueprint is never activated before it has been fully
///   transmitted. Displaying, or allowing a user to modify, a half-transmitted
///   blueprint can cause confusion and bad interactions with the space view heuristics.
/// - Additionally, this command allows fine-tuning the activation behavior itself
///   by specifying whether the blueprint should be immediately activated, or only
///   become the default for future activations.
#[derive(Clone, Debug, PartialEq, Eq)] // `PartialEq` used for tests in another crate
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BlueprintActivationCommand {
    /// The blueprint this command refers to.
    pub blueprint_id: StoreId,

    /// Immediately make this the active blueprint for the associated `app_id`.
    ///
    /// Note that setting this to `false` does not mean the blueprint may not still end
    /// up becoming active. In particular, if `make_default` is true and there is no other
    /// currently active blueprint.
    pub make_active: bool,

    /// Make this the default blueprint for the `app_id`.
    ///
    /// The default blueprint will be used as the template when the user resets the
    /// blueprint for the app. It will also become the active blueprint if no other
    /// blueprint is currently active.
    pub make_default: bool,
}

impl BlueprintActivationCommand {
    /// Make `blueprint_id` the default blueprint for its associated `app_id`.
    pub fn make_default(blueprint_id: StoreId) -> Self {
        Self {
            blueprint_id,
            make_active: false,
            make_default: true,
        }
    }

    /// Immediately make `blueprint_id` the active blueprint for its associated `app_id`.
    ///
    /// This also sets `make_default` to true.
    pub fn make_active(blueprint_id: StoreId) -> Self {
        Self {
            blueprint_id,
            make_active: true,
            make_default: true,
        }
    }
}

/// The most general log message sent from the SDK to the server.
#[must_use]
#[derive(Clone, Debug, PartialEq)] // `PartialEq` used for tests in another crate
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[allow(clippy::large_enum_variant)]
pub enum LogMsg {
    /// A new recording has begun.
    ///
    /// Should usually be the first message sent.
    SetStoreInfo(SetStoreInfo),

    /// Log an entity using an [`ArrowMsg`].
    ArrowMsg(StoreId, ArrowMsg),

    /// Send after all messages in a blueprint to signal that the blueprint is complete.
    ///
    /// This is so that the viewer can wait with activating the blueprint until it is
    /// fully transmitted. Showing a half-transmitted blueprint can cause confusion,
    /// and also lead to problems with space-view heuristics.
    BlueprintActivationCommand(BlueprintActivationCommand),
}

impl LogMsg {
    pub fn store_id(&self) -> &StoreId {
        match self {
            Self::SetStoreInfo(msg) => &msg.info.store_id,
            Self::ArrowMsg(store_id, _) => store_id,
            Self::BlueprintActivationCommand(cmd) => &cmd.blueprint_id,
        }
    }

    pub fn set_store_id(&mut self, new_store_id: StoreId) {
        match self {
            Self::SetStoreInfo(store_info) => {
                store_info.info.store_id = new_store_id;
            }
            Self::ArrowMsg(store_id, _) => {
                *store_id = new_store_id;
            }
            Self::BlueprintActivationCommand(cmd) => {
                cmd.blueprint_id = new_store_id;
            }
        }
    }
}

impl_into_enum!(SetStoreInfo, LogMsg, SetStoreInfo);
impl_into_enum!(
    BlueprintActivationCommand,
    LogMsg,
    BlueprintActivationCommand
);

// ----------------------------------------------------------------------------

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SetStoreInfo {
    pub row_id: RowId,
    pub info: StoreInfo,
}

/// Information about a recording or blueprint.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct StoreInfo {
    /// The user-chosen name of the application doing the logging.
    pub application_id: ApplicationId,

    /// Should be unique for each recording.
    pub store_id: StoreId,

    /// If this store is the result of a clone, which store was it cloned from?
    ///
    /// A cloned store always gets a new unique ID.
    ///
    /// We currently only clone stores for blueprints:
    /// when we receive a _default_ blueprints on the wire (e.g. from a recording),
    /// we clone it and make the clone the _active_ blueprint.
    /// This means all active blueprints are clones.
    pub cloned_from: Option<StoreId>,

    /// True if the recording is one of the official Rerun examples.
    pub is_official_example: bool,

    /// When the recording started.
    ///
    /// Should be an absolute time, i.e. relative to Unix Epoch.
    pub started: Time,

    pub store_source: StoreSource,

    /// The Rerun version used to encoded the RRD data.
    ///
    // NOTE: The version comes directly from the decoded RRD stream's header, duplicating it here
    // would probably only lead to more issues down the line.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub store_version: Option<CrateVersion>,
}

impl StoreInfo {
    /// Whether this `StoreInfo` is the default used when a user is not explicitly
    /// creating their own blueprint.
    pub fn is_app_default_blueprint(&self) -> bool {
        self.application_id.as_str() == self.store_id.as_str()
    }
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PythonVersion {
    /// e.g. 3
    pub major: u8,

    /// e.g. 11
    pub minor: u8,

    /// e.g. 0
    pub patch: u8,

    /// e.g. `a0` for alpha releases.
    pub suffix: String,
}

impl std::fmt::Debug for PythonVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::fmt::Display for PythonVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            major,
            minor,
            patch,
            suffix,
        } = self;
        write!(f, "{major}.{minor}.{patch}{suffix}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum FileSource {
    Cli,
    DragAndDrop,
    FileDialog,
    Sdk,
}

/// The source of a recording or blueprint.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum StoreSource {
    Unknown,

    /// The official Rerun C Logging SDK
    CSdk,

    /// The official Rerun Python Logging SDK
    PythonSdk(PythonVersion),

    /// The official Rerun Rust Logging SDK
    RustSdk {
        /// Rust version of the code compiling the Rust SDK
        rustc_version: String,

        /// LLVM version of the code compiling the Rust SDK
        llvm_version: String,
    },

    /// Loading a file via CLI, drag-and-drop, a file-dialog, etc.
    File {
        file_source: FileSource,
    },

    /// Generated from the viewer itself.
    Viewer,

    /// Perhaps from some manual data ingestion?
    Other(String),
}

impl std::fmt::Display for StoreSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => "Unknown".fmt(f),
            Self::CSdk => "C SDK".fmt(f),
            Self::PythonSdk(version) => write!(f, "Python {version} SDK"),
            Self::RustSdk { rustc_version, .. } => write!(f, "Rust SDK (rustc {rustc_version})"),
            Self::File { file_source, .. } => match file_source {
                FileSource::Cli => write!(f, "File via CLI"),
                FileSource::DragAndDrop => write!(f, "File via drag-and-drop"),
                FileSource::FileDialog => write!(f, "File via file dialog"),
                FileSource::Sdk => write!(f, "File via SDK"),
            },
            Self::Viewer => write!(f, "Viewer-generated"),
            Self::Other(string) => format!("{string:?}").fmt(f), // put it in quotes
        }
    }
}

// ---

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `log_time` suitable for inserting in a [`TimePoint`].
#[inline]
pub fn build_log_time(log_time: Time) -> (Timeline, TimeInt) {
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
        frame_nr.try_into().unwrap_or(TimeInt::MIN),
    )
}
