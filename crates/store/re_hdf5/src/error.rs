//! Error type for HDF5 loading.

/// Errors that can occur during HDF5 loading.
#[derive(Debug, thiserror::Error)]
pub enum Hdf5Error {
    //TODO(ab): these type-based error variants should be replaced by better error with context.
    // Doing so would incur quite a bit a churn and they should be rare in this crate, so postponed
    // for now.
    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Chunk construction: {0}")]
    Chunk(#[from] re_chunk::ChunkError),

    // --- HDF5-read failures, each naming the operation + in-file object. No file path — consumers add it. ---
    #[error("Failed to open the HDF5 file: {source}")]
    Open { source: hdf5_pure::Error },

    #[error("Failed to read the metadata of {path:?}: {source}")]
    Metadata {
        path: String,
        source: hdf5_pure::Error,
    },

    #[error("Failed to read dataset {path:?}: {source}")]
    ReadDataset {
        path: String,
        source: hdf5_pure::Error,
    },

    // --- "User configuration" errors → the PyO3 layer maps these to `ValueError`. ---
    #[error(
        "Datasets are not row-aligned to the {kind} of length {expected}: {offenders} \
         (add them to `ignore_datasets` or pick a compatible `index_column`)"
    )]
    RowAlignment {
        kind: &'static str,
        expected: u64,
        offenders: String,
    },

    #[error("Index dataset {path:?} not found in the file")]
    IndexNotFound { path: String },

    #[error("Index dataset {path:?} must be 1-dimensional, but has shape {shape:?}")]
    IndexNotOneDimensional { path: String, shape: Vec<u64> },

    #[error(
        "Index dataset {path:?} has non-numeric type {dtype}; a timeline requires a numeric dataset"
    )]
    IndexNotNumeric { path: String, dtype: String },

    // --- Missing object for the metadata accessors → the PyO3 layer maps this to `KeyError`. ---
    #[error("Object {path:?} not found in the file")]
    ObjectNotFound { path: String },

    #[error("Invalid component name {name:?}: {source}")]
    InvalidComponentName {
        name: String,
        source: re_sdk_types::InvalidComponentIdentifierError,
    },

    #[error("Invalid timeline name {name:?}: {source}")]
    InvalidTimelineName {
        name: String,
        source: re_log_types::InvalidTimelineNameError,
    },

    #[error("List length {length} exceeds the maximum supported size (i32::MAX)")]
    ListTooLong { length: u64 },

    #[error("Unsupported element type {dtype}")]
    UnsupportedElementType { dtype: String },
}

impl Hdf5Error {
    /// Wraps a file-open failure. Usable point-free: `.map_err(Hdf5Error::open)`.
    pub(crate) fn open(source: hdf5_pure::Error) -> Self {
        Self::Open { source }
    }

    /// A metadata-read failure of the object at `path`.
    pub(crate) fn metadata(path: impl std::fmt::Display, source: hdf5_pure::Error) -> Self {
        Self::Metadata {
            path: path.to_string(),
            source,
        }
    }

    /// A value-read failure of the dataset at `path`.
    pub(crate) fn read_dataset(path: impl std::fmt::Display, source: hdf5_pure::Error) -> Self {
        Self::ReadDataset {
            path: path.to_string(),
            source,
        }
    }

    /// An invalid component name.
    pub(crate) fn invalid_component_name(
        name: impl std::fmt::Display,
        source: re_sdk_types::InvalidComponentIdentifierError,
    ) -> Self {
        Self::InvalidComponentName {
            name: name.to_string(),
            source,
        }
    }

    /// An invalid timeline name.
    pub(crate) fn invalid_timeline_name(
        name: impl std::fmt::Display,
        source: re_log_types::InvalidTimelineNameError,
    ) -> Self {
        Self::InvalidTimelineName {
            name: name.to_string(),
            source,
        }
    }

    /// True for the "bad user config" variants
    ///
    /// These typically surfaces as Python `ValueError`.
    pub fn is_config_error(&self) -> bool {
        match self {
            Self::RowAlignment { .. }
            | Self::IndexNotFound { .. }
            | Self::IndexNotOneDimensional { .. }
            | Self::IndexNotNumeric { .. } => true,

            Self::Arrow(_)
            | Self::Chunk(_)
            | Self::Open { .. }
            | Self::Metadata { .. }
            | Self::ReadDataset { .. }
            | Self::ObjectNotFound { .. }
            | Self::InvalidComponentName { .. }
            | Self::InvalidTimelineName { .. }
            | Self::ListTooLong { .. }
            | Self::UnsupportedElementType { .. } => false,
        }
    }

    /// True for the "object not found" variant.
    ///
    /// These typically surface as Python `KeyError`.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::ObjectNotFound { .. })
    }
}
