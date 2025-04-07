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
mod entry_id;
pub mod example_components;
pub mod hash;
mod index;
pub mod path;

// mod data_cell;
// mod data_row;
// mod data_table;
mod instance;
mod vec_deque_ext;

use std::sync::Arc;

use re_build_info::CrateVersion;
use re_byte_size::SizeBytes;

pub use self::{
    arrow_msg::{ArrowMsg, ArrowRecordBatchReleaseCallback},
    entry_id::EntryId,
    index::{
        Duration, NonMinI64, ResolvedTimeRange, ResolvedTimeRangeF, TimeCell, TimeInt, TimePoint,
        TimeReal, TimeType, Timeline, TimelineName, Timestamp, TimestampFormat, TryFromIntError,
    },
    instance::Instance,
    path::*,
    vec_deque_ext::{VecDequeInsertionExt, VecDequeRemovalExt, VecDequeSortingExt},
};

pub mod external {
    pub use arrow;

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

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Clone, Debug, PartialEq, Eq)] // `PartialEq` used for tests in another crate
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
#[allow(clippy::large_enum_variant)]
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

    // TODO(#3741): remove this once we are all in on arrow-rs
    /// USE ONLY FOR TESTS
    pub fn strip_arrow_extension_types(self) -> Self {
        match self {
            Self::ArrowMsg(store_id, mut arrow_msg) => {
                strip_arrow_extension_types_from_batch(&mut arrow_msg.batch);
                Self::ArrowMsg(store_id, arrow_msg)
            }
            other => other,
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

    pub store_source: StoreSource,

    /// The Rerun version used to encoded the RRD data.
    ///
    // NOTE: The version comes directly from the decoded RRD stream's header, duplicating it here
    // would probably only lead to more issues down the line.
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FileSource {
    Cli,

    /// The user clicked on a recording URI in the viewer.
    Uri,

    DragAndDrop {
        /// The [`ApplicationId`] that the viewer heuristically recommends should be used when loading
        /// this data source, based on the surrounding context.
        recommended_application_id: Option<ApplicationId>,

        /// The [`StoreId`] that the viewer heuristically recommends should be used when loading
        /// this data source, based on the surrounding context.
        recommended_recording_id: Option<StoreId>,

        /// Whether `SetStoreInfo`s should be sent, regardless of the surrounding context.
        ///
        /// Only useful when creating a recording just-in-time directly in the viewer (which is what
        /// happens when importing things into the welcome screen).
        force_store_info: bool,
    },

    FileDialog {
        /// The [`ApplicationId`] that the viewer heuristically recommends should be used when loading
        /// this data source, based on the surrounding context.
        recommended_application_id: Option<ApplicationId>,

        /// The [`StoreId`] that the viewer heuristically recommends should be used when loading
        /// this data source, based on the surrounding context.
        recommended_recording_id: Option<StoreId>,

        /// Whether `SetStoreInfo`s should be sent, regardless of the surrounding context.
        ///
        /// Only useful when creating a recording just-in-time directly in the viewer (which is what
        /// happens when importing things into the welcome screen).
        force_store_info: bool,
    },

    Sdk,
}

impl FileSource {
    #[inline]
    pub fn recommended_application_id(&self) -> Option<&ApplicationId> {
        match self {
            Self::FileDialog {
                recommended_application_id,
                ..
            }
            | Self::DragAndDrop {
                recommended_application_id,
                ..
            } => recommended_application_id.as_ref(),
            Self::Cli | Self::Uri | Self::Sdk => None,
        }
    }

    #[inline]
    pub fn recommended_recording_id(&self) -> Option<&StoreId> {
        match self {
            Self::FileDialog {
                recommended_recording_id,
                ..
            }
            | Self::DragAndDrop {
                recommended_recording_id,
                ..
            } => recommended_recording_id.as_ref(),
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct TableMsg {
    /// The id of the table.
    pub id: TableId,

    /// The table stored as an [`ArrowRecordBatch`].
    pub data: ArrowRecordBatch,
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

impl SizeBytes for ApplicationId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

impl SizeBytes for StoreId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.id.heap_size_bytes()
    }
}

impl SizeBytes for PythonVersion {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            major: _,
            minor: _,
            patch: _,
            suffix,
        } = self;

        suffix.heap_size_bytes()
    }
}

impl SizeBytes for FileSource {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Uri | Self::Sdk | Self::Cli => 0,
            Self::DragAndDrop {
                recommended_application_id,
                recommended_recording_id,
                force_store_info,
            }
            | Self::FileDialog {
                recommended_application_id,
                recommended_recording_id,
                force_store_info,
            } => {
                recommended_application_id.heap_size_bytes()
                    + recommended_recording_id.heap_size_bytes()
                    + force_store_info.heap_size_bytes()
            }
        }
    }
}

impl SizeBytes for StoreSource {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::Unknown | Self::CSdk | Self::Viewer => 0,
            Self::PythonSdk(python_version) => python_version.heap_size_bytes(),
            Self::RustSdk {
                rustc_version,
                llvm_version,
            } => rustc_version.heap_size_bytes() + llvm_version.heap_size_bytes(),
            Self::File { file_source } => file_source.heap_size_bytes(),
            Self::Other(description) => description.heap_size_bytes(),
        }
    }
}

impl SizeBytes for StoreInfo {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            application_id,
            store_id,
            cloned_from: _,
            store_source,
            store_version,
        } = self;

        application_id.heap_size_bytes()
            + store_id.heap_size_bytes()
            + store_source.heap_size_bytes()
            + store_version.heap_size_bytes()
    }
}

impl SizeBytes for SetStoreInfo {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { row_id, info } = self;

        row_id.heap_size_bytes() + info.heap_size_bytes()
    }
}

impl SizeBytes for BlueprintActivationCommand {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl SizeBytes for ArrowMsg {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            chunk_id,
            timepoint_max,
            batch,
            on_release: _,
        } = self;

        chunk_id.heap_size_bytes() + timepoint_max.heap_size_bytes() + batch.heap_size_bytes()
    }
}

impl SizeBytes for LogMsg {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::SetStoreInfo(set_store_info) => set_store_info.heap_size_bytes(),
            Self::ArrowMsg(store_id, arrow_msg) => {
                store_id.heap_size_bytes() + arrow_msg.heap_size_bytes()
            }
            Self::BlueprintActivationCommand(blueprint_activation_command) => {
                blueprint_activation_command.heap_size_bytes()
            }
        }
    }
}

/// USE ONLY FOR TESTS
// TODO(#3741): remove once <https://github.com/apache/arrow-rs/issues/6803> is released
use arrow::{array::RecordBatch as ArrowRecordBatch, datatypes::Field, error::ArrowError};

pub fn strip_arrow_extension_types_from_batch(batch: &mut ArrowRecordBatch) {
    use arrow::datatypes::{Field, Schema};

    fn strip_arrow_extensions_from_field(field: &Field) -> Field {
        let mut metadata = field.metadata().clone();
        metadata.retain(|key, _| !key.starts_with("ARROW:extension"));
        field.clone().with_metadata(metadata)
    }

    let old_schema = batch.schema();
    let new_fields: arrow::datatypes::Fields = old_schema
        .fields()
        .iter()
        .map(|field| strip_arrow_extensions_from_field(field))
        .collect();
    let new_schema = Schema::new_with_metadata(new_fields, old_schema.metadata().clone());

    #[allow(clippy::unwrap_used)] // The invariants of the input aren't changed
    {
        *batch = ArrowRecordBatch::try_new(new_schema.into(), batch.columns().to_vec()).unwrap();
    }
}

// ----------------------------------------------------------------------------

/// Runtime asserts that an archetype has the given components.
///
/// In particular, this is useful to statically check that an archetype
/// has a specific component.
///
/// ```
/// # #[macro_use] extern crate re_log_types;
/// # use re_log_types::example_components::*;
/// debug_assert_archetype_has_components!(MyPoints, colors: MyColor);
/// ```
///
/// This will panic because the type is wrong:
///
/// ```should_panic
/// # #[macro_use] extern crate re_log_types;
/// # use re_log_types::example_components::*;
/// debug_assert_archetype_has_components!(MyPoints, colors: MyPoint);
/// ```
///
/// This will fail to compile because the field is missing:
///
/// ```compile_fail
/// # #[macro_use] extern crate re_log_types;
/// # use re_log_types::example_components::*;
/// debug_assert_archetype_has_components!(MyPoints, colours: MyColor);
/// ```
///
#[macro_export]
macro_rules! debug_assert_archetype_has_components {
    ($arch:ty, $($field:ident: $field_typ:ty),+ $(,)?) => {
        #[cfg(debug_assertions)]
        {
            use re_log_types::external::re_types_core::{Component as _};
            let archetype = <$arch>::clear_fields();
            $(
                assert_eq!(archetype.$field.map(|batch| batch.descriptor.component_name), Some(<$field_typ>::name()));
            )+
        }
    };
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

    #[test]
    fn table_msg_concatenated_roundtrip() {
        use arrow::{
            array::{ArrayRef, StringArray, UInt64Array},
            datatypes::{DataType, Field, Schema},
        };

        let data = {
            let schema = Arc::new(Schema::new_with_metadata(
                vec![
                    Field::new("id", DataType::UInt64, false),
                    Field::new("name", DataType::Utf8, false),
                ],
                Default::default(),
            ));

            // Create a UInt64 array
            let id_array = UInt64Array::from(vec![1, 2, 3, 4, 5]);

            // Create a String array
            let name_array = StringArray::from(vec![
                "Alice",
                "Bob",
                "Charlie",
                "Dave",
                "http://www.rerun.io",
            ]);

            // Convert arrays to ArrayRef (trait objects)
            let arrays: Vec<ArrayRef> = vec![
                Arc::new(id_array) as ArrayRef,
                Arc::new(name_array) as ArrayRef,
            ];

            // Create a RecordBatch
            ArrowRecordBatch::try_new(schema, arrays).unwrap()
        };

        let msg = TableMsg {
            id: TableId::new("test123".to_owned()),
            data,
        };

        let encoded = msg.to_arrow_encoded().expect("to encoded failed");
        let decoded = TableMsg::from_arrow_encoded(&encoded).expect("from concatenated failed");

        assert_eq!(msg, decoded);
    }
}
