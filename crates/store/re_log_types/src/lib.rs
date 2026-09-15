//! The basic building blocks of the Rerun log format: entity paths, timelines, store ids, and log messages.
//!
//! An entity path names *what* was logged, a [`TimePoint`] on one or more timelines names *when*,
//! and a [`LogMsg`] is the envelope that carries it between the SDK, an `.rrd` file, and the viewer.
//!
//! The data itself is described by `re_sdk_types`.
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
pub mod arrow_msg;
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

use arrow::array::RecordBatch as ArrowRecordBatch;
use re_build_info::CrateVersion;

pub use re_types_core::{InvalidTimelineNameError, TimelineName};

pub use self::app_id::{ApplicationId, InvalidApplicationIdError};
pub use self::arrow_msg::{ArrowMsg, ArrowRecordBatchReleaseCallback};
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

// ----------------------------------------------------------------------------

/// Command used for activating a blueprint once it has been fully transmitted.
///
/// This command serves two purposes:
/// - It is important that a blueprint is never activated before it has been fully
///   transmitted. Displaying, or allowing a user to modify, a half-transmitted
///   blueprint can cause confusion and bad interactions with the view heuristics.
/// - Additionally, this command allows fine-tuning the activation behavior itself
///   by specifying whether the blueprint should be immediately activated, or only
///   become the default for future activations.
#[derive(Clone, Debug, PartialEq, Eq, re_byte_size::SizeBytes)] // `PartialEq` used for tests in another crate
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
///
/// Note: this does not contain tables sent via [`TableMsg`], as these concepts are fundamentally
/// different and should not be handled uniformly. For example, we don't want to store tables in
/// `.rrd` files.
#[must_use]
#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)] // `PartialEq` used for tests in another crate
// TODO(#8631): Remove `LogMsg`
pub enum LogMsg {
    /// A new recording has begun.
    ///
    /// Should usually be the first message sent.
    SetStoreInfo(SetStoreInfo),

    /// Log an entity using an [`ArrowMsg`].
    //
    // TODO(#6574): the store ID should be in the metadata here so we can remove the layer on top
    ArrowMsg(StoreId, ArrowMsg),

    /// Send after all messages in a blueprint to signal that the blueprint is complete.
    ///
    /// This is so that the viewer can wait with activating the blueprint until it is
    /// fully transmitted. Showing a half-transmitted blueprint can cause confusion,
    /// and also lead to problems with view heuristics.
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

    /// If we are an [`ArrowMsg`], return a mutable reference to the underlying
    /// [`ArrowRecordBatch`].
    pub fn arrow_record_batch_mut(&mut self) -> Option<&mut ArrowRecordBatch> {
        match self {
            Self::ArrowMsg(_, arrow_msg) => Some(&mut arrow_msg.batch),
            _ => None,
        }
    }

    pub fn insert_arrow_record_batch_metadata(&mut self, key: String, value: String) {
        if let Some(record_batch) = self.arrow_record_batch_mut() {
            record_batch.schema_metadata_mut().insert(key, value);
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
#[derive(Clone, Debug, PartialEq, Eq, re_byte_size::SizeBytes)]
pub struct SetStoreInfo {
    /// A time-based UID that is only used to help keep track of when these `StoreInfo` originated
    /// and how they fit in the global ordering of events.
    //
    // NOTE: Using a raw `Tuid` instead of an actual `RowId` to prevent a nasty dependency cycle.
    // Note that both using a `RowId` as well as this whole serde/msgpack layer as a whole are hacks
    // that are destined to disappear anyhow as we are closing in on our network-exposed data APIs.
    pub row_id: re_tuid::Tuid,

    pub info: StoreInfo,
}

/// Information about a recording or blueprint.
#[derive(Clone, Debug, PartialEq, Eq, re_byte_size::SizeBytes)]
pub struct StoreInfo {
    /// Should be unique for each recording.
    ///
    /// The store id contains both the application id (or dataset id) and the recording id (or
    /// segment id).
    pub store_id: StoreId,

    pub store_source: StoreSource,

    /// The Rerun version used to encoded the RRD data.
    ///
    // NOTE: The version comes directly from the decoded RRD stream's header, duplicating it here
    // would probably only lead to more issues down the line.
    pub store_version: Option<CrateVersion<'static>>,
}

impl StoreInfo {
    /// Creates a new store info using the current crate version.
    pub fn new(store_id: StoreId, store_source: StoreSource) -> Self {
        Self {
            store_id,
            store_source,
            store_version: Some(CrateVersion::LOCAL),
        }
    }

    /// Creates a new store info without any versioning information.
    pub fn new_unversioned(store_id: StoreId, store_source: StoreSource) -> Self {
        Self {
            store_id,
            store_source,
            store_version: None,
        }
    }

    /// Creates a new store info for testing purposes.
    pub fn testing() -> Self {
        // Do not use a version since it breaks snapshot tests on every update otherwise.
        Self::new_unversioned(
            StoreId::random(StoreKind::Recording, "test_app"),
            StoreSource::Other("test".to_owned()),
        )
    }

    /// Creates a new store info for testing purposes with a fixed store id.
    ///
    /// Most of the time we don't want to fix the store id since it is used as a key in static store subscribers, which might not get teared down after every test.
    /// Use this only if the recording id may show up somewhere in the test output.
    pub fn testing_with_recording_id(recording_id: impl Into<RecordingId>) -> Self {
        // Do not use a version since it breaks snapshot tests on every update otherwise.
        Self::new_unversioned(
            StoreId::new(StoreKind::Recording, "test_app", recording_id),
            StoreSource::Other("test".to_owned()),
        )
    }
}

impl StoreInfo {
    /// Whether this `StoreInfo` is the default used when a user is not explicitly
    /// creating their own blueprint.
    pub fn is_app_default_blueprint(&self) -> bool {
        self.application_id().as_str() == self.recording_id().as_str()
    }

    pub fn application_id(&self) -> &ApplicationId {
        self.store_id.application_id()
    }

    pub fn recording_id(&self) -> &RecordingId {
        self.store_id.recording_id()
    }
}

#[derive(Clone, PartialEq, Eq, re_byte_size::SizeBytes)]
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

impl std::str::FromStr for PythonVersion {
    type Err = PythonVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(PythonVersionParseError::MissingMajor);
        }
        let (major, rest) = s
            .split_once('.')
            .ok_or(PythonVersionParseError::MissingMinor)?;
        if rest.is_empty() {
            return Err(PythonVersionParseError::MissingMinor);
        }
        let (minor, rest) = rest
            .split_once('.')
            .ok_or(PythonVersionParseError::MissingPatch)?;
        if rest.is_empty() {
            return Err(PythonVersionParseError::MissingPatch);
        }
        let pos = rest.bytes().position(|v| !v.is_ascii_digit());
        let (patch, suffix) = match pos {
            Some(pos) => rest.split_at(pos),
            None => (rest, ""),
        };

        Ok(Self {
            major: major
                .parse()
                .map_err(PythonVersionParseError::InvalidMajor)?,
            minor: minor
                .parse()
                .map_err(PythonVersionParseError::InvalidMinor)?,
            patch: patch
                .parse()
                .map_err(PythonVersionParseError::InvalidPatch)?,
            suffix: suffix.into(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PythonVersionParseError {
    #[error("missing major version")]
    MissingMajor,

    #[error("missing minor version")]
    MissingMinor,

    #[error("missing patch version")]
    MissingPatch,

    #[error("invalid major version: {0}")]
    InvalidMajor(std::num::ParseIntError),

    #[error("invalid minor version: {0}")]
    InvalidMinor(std::num::ParseIntError),

    #[error("invalid patch version: {0}")]
    InvalidPatch(std::num::ParseIntError),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub enum FileSource {
    Cli,

    /// The user clicked on a recording URI in the viewer.
    Uri,

    DragAndDrop {
        /// The [`StoreId`] that the viewer heuristically recommends should be used when loading
        /// this data source, based on the surrounding context.
        recommended_store_id: Option<StoreId>,

        /// Whether `SetStoreInfo`s should be sent, regardless of the surrounding context.
        ///
        /// Only useful when creating a recording just-in-time directly in the viewer (which is what
        /// happens when importing things into the welcome screen).
        force_store_info: bool,
    },

    FileDialog {
        /// The [`StoreId`] that the viewer heuristically recommends should be used when loading
        /// this data source, based on the surrounding context.
        recommended_store_id: Option<StoreId>,

        /// Whether `SetStoreInfo`s should be sent, regardless of the surrounding context.
        ///
        /// Only useful when creating a recording just-in-time directly in the viewer (which is what
        /// happens when importing things into the welcome screen).
        force_store_info: bool,
    },

    Sdk,
}

impl FileSource {
    pub fn recommended_store_id(&self) -> Option<&StoreId> {
        match self {
            Self::FileDialog {
                recommended_store_id,
                ..
            }
            | Self::DragAndDrop {
                recommended_store_id,
                ..
            } => recommended_store_id.as_ref(),
            Self::Cli | Self::Uri | Self::Sdk => None,
        }
    }

    #[inline]
    pub fn force_store_info(&self) -> bool {
        match self {
            Self::FileDialog {
                force_store_info, ..
            }
            | Self::DragAndDrop {
                force_store_info, ..
            } => *force_store_info,
            Self::Cli | Self::Uri | Self::Sdk => false,
        }
    }
}

/// The source of a recording or blueprint.
#[derive(Clone, Debug, PartialEq, Eq, re_byte_size::SizeBytes)]
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
                FileSource::Uri => write!(f, "File via URI"),
                FileSource::DragAndDrop { .. } => write!(f, "File via drag-and-drop"),
                FileSource::FileDialog { .. } => write!(f, "File via file dialog"),
                FileSource::Sdk => write!(f, "File via SDK"),
            },
            Self::Viewer => write!(f, "Viewer-generated"),
            Self::Other(string) => format!("{string:?}").fmt(f), // put it in quotes
        }
    }
}

// ---

/// A table, encoded as a dataframe of Arrow record batches.
///
/// Tables have a [`TableId`], but don't belong to an application and therefore don't have an [`ApplicationId`].
/// For now, the table is always sent as a whole, i.e. tables can't be streamed.
///
/// It's important to note that tables are not sent via the smart channel of [`LogMsg`], but use a separate `crossbeam`
/// channel. The reasoning behind this is that tables are fundamentally different from recordings. For example,
/// we don't want to store tables in `.rrd` files, as there are much better formats out there.
#[must_use]
#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)]
pub struct TableMsg {
    /// The id of the table.
    pub id: TableId,

    /// The table stored as an [`ArrowRecordBatch`].
    pub data: ArrowRecordBatch,
}

impl TableMsg {
    pub fn insert_arrow_record_batch_metadata(&mut self, key: String, value: String) {
        self.data.schema_metadata_mut().insert(key, value);
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

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_python_version() {
        macro_rules! assert_parse_err {
            ($input:literal, $expected:pat) => {
                let actual = $input.parse::<PythonVersion>();

                assert!(
                    matches!(actual, Err($expected)),
                    "actual: {actual:?}, expected: {}",
                    stringify!($expected)
                );
            };
        }

        macro_rules! assert_parse_ok {
            ($input:literal, $expected:expr) => {
                let actual = $input.parse::<PythonVersion>().expect("failed to parse");
                assert_eq!(actual, $expected);
            };
        }

        assert_parse_err!("", PythonVersionParseError::MissingMajor);
        assert_parse_err!("3", PythonVersionParseError::MissingMinor);
        assert_parse_err!("3.", PythonVersionParseError::MissingMinor);
        assert_parse_err!("3.11", PythonVersionParseError::MissingPatch);
        assert_parse_err!("3.11.", PythonVersionParseError::MissingPatch);
        assert_parse_err!("a.11.0", PythonVersionParseError::InvalidMajor(_));
        assert_parse_err!("3.b.0", PythonVersionParseError::InvalidMinor(_));
        assert_parse_err!("3.11.c", PythonVersionParseError::InvalidPatch(_));
        assert_parse_ok!(
            "3.11.0",
            PythonVersion {
                major: 3,
                minor: 11,
                patch: 0,
                suffix: String::new(),
            }
        );
        assert_parse_ok!(
            "3.11.0a1",
            PythonVersion {
                major: 3,
                minor: 11,
                patch: 0,
                suffix: "a1".to_owned(),
            }
        );
    }
}
