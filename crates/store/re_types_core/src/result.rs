use std::any;
use std::fmt::Display;
use std::ops::Deref;

// ---

// NOTE: We have to make an alias, otherwise we'll trigger `thiserror`'s magic codepath which will
// attempt to use nightly features.
pub type _Backtrace = std::backtrace::Backtrace;

#[derive(thiserror::Error)]
pub enum SerializationError {
    #[error("Failed to serialize {location:?}")]
    Context { location: String, source: Box<Self> },

    #[error("Trying to serialize a field lacking extension metadata: {fqname:?}")]
    MissingExtensionMetadata {
        fqname: String,
        backtrace: Box<_Backtrace>,
    },

    #[error("{fqname} doesn't support Serialization: {reason}")]
    NotImplemented {
        fqname: String,
        reason: String,
        backtrace: Box<_Backtrace>,
    },

    /// E.g. too many values (overflows i32).
    #[error(transparent)]
    ArrowError(#[from] ArcArrowError),
}

#[test]
fn test_serialization_error_size() {
    assert!(
        std::mem::size_of::<SerializationError>() <= 64,
        "Size of error is {} bytes. Let's try to keep errors small.",
        std::mem::size_of::<SerializationError>()
    );
}

impl std::fmt::Debug for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(backtrace) = self.backtrace() {
            f.write_fmt(format_args!("{self}:\n{backtrace:#?}"))
        } else {
            f.write_fmt(format_args!("{self}"))
        }
    }
}

impl SerializationError {
    #[inline]
    pub fn missing_extension_metadata(fqname: impl AsRef<str>) -> Self {
        Self::MissingExtensionMetadata {
            fqname: fqname.as_ref().into(),
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn not_implemented(fqname: impl AsRef<str>, reason: impl AsRef<str>) -> Self {
        Self::NotImplemented {
            fqname: fqname.as_ref().into(),
            reason: reason.as_ref().into(),
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    /// Returns the _unresolved_ backtrace associated with this error, if it exists.
    ///
    /// Call `resolve()` on the returned [`_Backtrace`] to resolve it (costly!).
    pub fn backtrace(&self) -> Option<&_Backtrace> {
        match self {
            Self::MissingExtensionMetadata { backtrace, .. }
            | Self::NotImplemented { backtrace, .. } => Some(backtrace),
            Self::ArrowError { .. } | Self::Context { .. } => None,
        }
    }
}

// ----------------------------------------------------------------------------

/// A cloneable wrapper around [`arrow::error::ArrowError`], for easier use.
///
/// The motivation behind this type is that we often use code that can return a [`arrow::error::ArrowError`]
/// inside functions that return a `SerializationError`. By wrapping it we can use the ? operator and simplify the code.
/// Second, normally also [`arrow::error::ArrowError`] isn't cloneable, but `SerializationError` is.
#[derive(Clone, Debug)]
pub struct ArcArrowError(std::sync::Arc<arrow::error::ArrowError>);

impl From<arrow::error::ArrowError> for ArcArrowError {
    fn from(e: arrow::error::ArrowError) -> Self {
        Self(std::sync::Arc::new(e))
    }
}

impl From<arrow::error::ArrowError> for SerializationError {
    fn from(e: arrow::error::ArrowError) -> Self {
        Self::ArrowError(ArcArrowError::from(e))
    }
}

impl Deref for ArcArrowError {
    type Target = arrow::error::ArrowError;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Display for ArcArrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// ----------------------------------------------------------------------------

pub type SerializationResult<T> = ::std::result::Result<T, SerializationError>;

// ---

#[derive(thiserror::Error)]
pub enum DeserializationError {
    #[error("Failed to deserialize {location:?}")]
    Context {
        location: String,
        #[source]
        source: Box<Self>,
    },

    #[error("{fqname} doesn't support deserialization")]
    NotImplemented {
        fqname: String,
        backtrace: Box<_Backtrace>,
    },

    #[error("Expected non-nullable data but didn't find any")]
    MissingData { backtrace: Box<_Backtrace> },

    #[error("Expected field {field_name:?} to be present in {datatype}")]
    MissingStructField {
        datatype: arrow::datatypes::DataType,
        field_name: String,
        backtrace: Box<_Backtrace>,
    },

    #[error(
        "Found {field1_length} {field1_name:?} values vs. {field2_length} {field2_name:?} values"
    )]
    MismatchedStructFieldLengths {
        field1_name: String,
        field1_length: usize,
        field2_name: String,
        field2_length: usize,
        backtrace: Box<_Backtrace>,
    },

    #[error("Expected union arm {arm_name:?} (#{arm_index}) to be present in {datatype}")]
    MissingUnionArm {
        datatype: arrow::datatypes::DataType,
        arm_name: String,
        arm_index: usize,
        backtrace: Box<_Backtrace>,
    },

    #[error("Expected {expected} but found {got} instead")]
    DatatypeMismatch {
        expected: arrow::datatypes::DataType,
        got: arrow::datatypes::DataType,
        backtrace: Box<_Backtrace>,
    },

    #[error("Offset ouf of bounds: trying to read at offset #{offset} in an array of size {len}")]
    OffsetOutOfBounds {
        offset: usize,
        len: usize,
        backtrace: Box<_Backtrace>,
    },

    #[error(
        "Offset slice ouf of bounds: trying to read offset slice at [#{from}..#{to}] in an array of size {len}"
    )]
    OffsetSliceOutOfBounds {
        from: usize,
        to: usize,
        len: usize,
        backtrace: Box<_Backtrace>,
    },

    #[error("Downcast to {to} failed")]
    DowncastError {
        to: String,
        backtrace: Box<_Backtrace>,
    },

    #[error("Datacell deserialization Failed: {0}")]
    DataCellError(String),

    #[error("Validation Error: {0}")]
    ValidationError(String),
}

#[test]
fn test_derserialization_error_size() {
    assert!(
        std::mem::size_of::<DeserializationError>() <= 72,
        "Size of error is {} bytes. Let's try to keep errors small.",
        std::mem::size_of::<DeserializationError>()
    );
}

impl std::fmt::Debug for DeserializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(backtrace) = self.backtrace() {
            f.write_fmt(format_args!("{self}:\n{backtrace:#?}"))
        } else {
            f.write_fmt(format_args!("{self}"))
        }
    }
}

impl DeserializationError {
    #[inline]
    pub fn missing_data() -> Self {
        Self::MissingData {
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn missing_struct_field(
        datatype: impl Into<arrow::datatypes::DataType>,
        field_name: impl AsRef<str>,
    ) -> Self {
        Self::MissingStructField {
            datatype: datatype.into(),
            field_name: field_name.as_ref().into(),
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn mismatched_struct_field_lengths(
        field1_name: impl AsRef<str>,
        field1_length: usize,
        field2_name: impl AsRef<str>,
        field2_length: usize,
    ) -> Self {
        Self::MismatchedStructFieldLengths {
            field1_name: field1_name.as_ref().into(),
            field1_length,
            field2_name: field2_name.as_ref().into(),
            field2_length,
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn missing_union_arm(
        datatype: impl Into<arrow::datatypes::DataType>,
        arm_name: impl AsRef<str>,
        arm_index: usize,
    ) -> Self {
        Self::MissingUnionArm {
            datatype: datatype.into(),
            arm_name: arm_name.as_ref().into(),
            arm_index,
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn datatype_mismatch(
        expected: impl Into<arrow::datatypes::DataType>,
        got: impl Into<arrow::datatypes::DataType>,
    ) -> Self {
        Self::DatatypeMismatch {
            expected: expected.into(),
            got: got.into(),
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn offset_oob(offset: usize, len: usize) -> Self {
        Self::OffsetOutOfBounds {
            offset,
            len,
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn offset_slice_oob((from, to): (usize, usize), len: usize) -> Self {
        Self::OffsetSliceOutOfBounds {
            from,
            to,
            len,
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    #[inline]
    pub fn downcast_error<ToType>() -> Self {
        Self::DowncastError {
            to: any::type_name::<ToType>().to_owned(),
            backtrace: Box::new(std::backtrace::Backtrace::capture()),
        }
    }

    /// Returns the _unresolved_ backtrace associated with this error, if it exists.
    ///
    /// Call `resolve()` on the returned [`_Backtrace`] to resolve it (costly!).
    #[inline]
    pub fn backtrace(&self) -> Option<&_Backtrace> {
        match self {
            Self::Context {
                location: _,
                source,
            } => source.backtrace(),
            Self::NotImplemented { backtrace, .. }
            | Self::MissingStructField { backtrace, .. }
            | Self::MismatchedStructFieldLengths { backtrace, .. }
            | Self::MissingUnionArm { backtrace, .. }
            | Self::MissingData { backtrace }
            | Self::DatatypeMismatch { backtrace, .. }
            | Self::OffsetOutOfBounds { backtrace, .. }
            | Self::OffsetSliceOutOfBounds { backtrace, .. }
            | Self::DowncastError { backtrace, .. } => Some(backtrace),
            Self::DataCellError(_) | Self::ValidationError(_) => None,
        }
    }

    /// The source of the error, without any [`Self::Context`].
    pub fn without_context(self) -> Self {
        match self {
            Self::Context { source, .. } => source.without_context(),
            _ => self,
        }
    }
}

pub type DeserializationResult<T> = ::std::result::Result<T, DeserializationError>;

pub trait ResultExt<T> {
    fn with_context(self, location: impl AsRef<str>) -> Self;
}

impl<T> ResultExt<T> for SerializationResult<T> {
    #[inline]
    fn with_context(self, location: impl AsRef<str>) -> Self {
        self.map_err(|err| SerializationError::Context {
            location: location.as_ref().into(),
            source: Box::new(err),
        })
    }
}

impl<T> ResultExt<T> for DeserializationResult<T> {
    #[inline]
    fn with_context(self, location: impl AsRef<str>) -> Self {
        self.map_err(|err| DeserializationError::Context {
            location: location.as_ref().into(),
            source: Box::new(err),
        })
    }
}
