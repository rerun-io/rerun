use re_build_info::CrateVersion;
use re_log_types::{ApplicationId, RecordingId, StoreId, StoreKind};

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

    /// The Rerun version used to encode the RRD data.
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
