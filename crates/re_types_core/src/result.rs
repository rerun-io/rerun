use crate::ComponentName;

// ---

// NOTE: We have to make an alias, otherwise we'll trigger `thiserror`'s magic codepath which will
// attempt to use nightly features.
pub type _Backtrace = backtrace::Backtrace;

#[derive(thiserror::Error, Clone)]
pub enum SerializationError {
    #[error("Failed to serialize {location:?}")]
    Context {
        location: String,
        source: Box<SerializationError>,
    },

    #[error("Trying to serialize a field lacking extension metadata: {fqname:?}")]
    MissingExtensionMetadata {
        fqname: String,
        backtrace: _Backtrace,
    },

    #[error("arrow2-convert serialization failed: {0}")]
    ArrowConvertFailure(String),

    #[error("serde-based serialization (`attr.rust.serde_type`) failed: {reason}")]
    SerdeFailure {
        reason: String,
        backtrace: _Backtrace,
    },

    #[error("{fqname} doesn't support deserialization: {reason}")]
    NotImplemented {
        fqname: String,
        reason: String,
        backtrace: _Backtrace,
    },
}

impl std::fmt::Debug for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bt = self.backtrace().map(|mut bt| {
            bt.resolve();
            bt
        });

        let err = Box::new(self.clone()) as Box<dyn std::error::Error>;
        if let Some(bt) = bt {
            f.write_fmt(format_args!("{}:\n{:#?}", re_error::format(&err), bt))
        } else {
            f.write_fmt(format_args!("{}", re_error::format(&err)))
        }
    }
}

impl SerializationError {
    #[inline]
    pub fn missing_extension_metadata(fqname: impl AsRef<str>) -> Self {
        Self::MissingExtensionMetadata {
            fqname: fqname.as_ref().into(),
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn serde_failure(reason: impl AsRef<str>) -> Self {
        Self::SerdeFailure {
            reason: reason.as_ref().into(),
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn not_implemented(fqname: impl AsRef<str>, reason: impl AsRef<str>) -> Self {
        Self::NotImplemented {
            fqname: fqname.as_ref().into(),
            reason: reason.as_ref().into(),
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    /// Returns the _unresolved_ backtrace associated with this error, if it exists.
    ///
    /// Call `resolve()` on the returned [`_Backtrace`] to resolve it (costly!).
    pub fn backtrace(&self) -> Option<_Backtrace> {
        match self {
            Self::MissingExtensionMetadata { backtrace, .. }
            | Self::SerdeFailure { backtrace, .. }
            | Self::NotImplemented { backtrace, .. } => Some(backtrace.clone()),
            SerializationError::Context { .. } | SerializationError::ArrowConvertFailure(_) => None,
        }
    }
}

pub type SerializationResult<T> = ::std::result::Result<T, SerializationError>;

// ---

#[derive(thiserror::Error, Clone)]
pub enum DeserializationError {
    #[error("Failed to deserialize {location:?}")]
    Context {
        location: String,
        #[source]
        source: Box<DeserializationError>,
    },

    #[error("{fqname} doesn't support deserialization")]
    NotImplemented {
        fqname: String,
        backtrace: _Backtrace,
    },

    #[error("Expected non-nullable data but didn't find any")]
    MissingData { backtrace: _Backtrace },

    #[error("Expected non-nullable data but didn't find any for component {component}")]
    MissingComponent {
        component: ComponentName,
        backtrace: _Backtrace,
    },

    #[error("Expected field {field_name:?} to be present in {datatype:#?}")]
    MissingStructField {
        datatype: ::arrow2::datatypes::DataType,
        field_name: String,
        backtrace: _Backtrace,
    },

    #[error(
        "Found {field1_length} {field1_name:?} values vs. {field2_length} {field2_name:?} values"
    )]
    MismatchedStructFieldLengths {
        field1_name: String,
        field1_length: usize,
        field2_name: String,
        field2_length: usize,
        backtrace: _Backtrace,
    },

    #[error("Expected union arm {arm_name:?} (#{arm_index}) to be present in {datatype:#?}")]
    MissingUnionArm {
        datatype: ::arrow2::datatypes::DataType,
        arm_name: String,
        arm_index: usize,
        backtrace: _Backtrace,
    },

    #[error("Expected {expected:#?} but found {got:#?} instead")]
    DatatypeMismatch {
        expected: ::arrow2::datatypes::DataType,
        got: ::arrow2::datatypes::DataType,
        backtrace: _Backtrace,
    },

    #[error("Offset ouf of bounds: trying to read at offset #{offset} in an array of size {len}")]
    OffsetOutOfBounds {
        offset: usize,
        len: usize,
        backtrace: _Backtrace,
    },

    #[error(
        "Offset slice ouf of bounds: trying to read offset slice at [#{from}..#{to}] in an array of size {len}"
    )]
    OffsetSliceOutOfBounds {
        from: usize,
        to: usize,
        len: usize,
        backtrace: _Backtrace,
    },

    #[error("arrow2-convert deserialization Failed: {0}")]
    ArrowConvertFailure(String),

    #[error("serde-based deserialization (`attr.rust.serde_type`) failed: {reason}")]
    SerdeFailure {
        reason: String,
        backtrace: _Backtrace,
    },

    #[error("Datacell deserialization Failed: {0}")]
    DataCellError(String),

    #[error("Validation Error: {0}")]
    ValidationError(String),
}

impl std::fmt::Debug for DeserializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bt = self.backtrace().map(|mut bt| {
            bt.resolve();
            bt
        });

        let err = Box::new(self.clone()) as Box<dyn std::error::Error>;
        if let Some(bt) = bt {
            f.write_fmt(format_args!("{}:\n{:#?}", re_error::format(&err), bt))
        } else {
            f.write_fmt(format_args!("{}", re_error::format(&err)))
        }
    }
}

impl DeserializationError {
    #[inline]
    pub fn missing_data() -> Self {
        Self::MissingData {
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn missing_struct_field(
        datatype: arrow2::datatypes::DataType,
        field_name: impl AsRef<str>,
    ) -> Self {
        Self::MissingStructField {
            datatype,
            field_name: field_name.as_ref().into(),
            backtrace: ::backtrace::Backtrace::new_unresolved(),
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
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn missing_union_arm(
        datatype: arrow2::datatypes::DataType,
        arm_name: impl AsRef<str>,
        arm_index: usize,
    ) -> Self {
        Self::MissingUnionArm {
            datatype,
            arm_name: arm_name.as_ref().into(),
            arm_index,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn datatype_mismatch(
        expected: arrow2::datatypes::DataType,
        got: arrow2::datatypes::DataType,
    ) -> Self {
        Self::DatatypeMismatch {
            expected,
            got,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn offset_oob(offset: usize, len: usize) -> Self {
        Self::OffsetOutOfBounds {
            offset,
            len,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn offset_slice_oob((from, to): (usize, usize), len: usize) -> Self {
        Self::OffsetSliceOutOfBounds {
            from,
            to,
            len,
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    #[inline]
    pub fn serde_failure(reason: impl AsRef<str>) -> Self {
        Self::SerdeFailure {
            reason: reason.as_ref().into(),
            backtrace: ::backtrace::Backtrace::new_unresolved(),
        }
    }

    /// Returns the _unresolved_ backtrace associated with this error, if it exists.
    ///
    /// Call `resolve()` on the returned [`_Backtrace`] to resolve it (costly!).
    #[inline]
    pub fn backtrace(&self) -> Option<_Backtrace> {
        match self {
            DeserializationError::Context {
                location: _,
                source,
            } => source.backtrace(),
            DeserializationError::NotImplemented { backtrace, .. }
            | DeserializationError::MissingStructField { backtrace, .. }
            | DeserializationError::MismatchedStructFieldLengths { backtrace, .. }
            | DeserializationError::MissingUnionArm { backtrace, .. }
            | DeserializationError::MissingData { backtrace }
            | DeserializationError::MissingComponent { backtrace, .. }
            | DeserializationError::DatatypeMismatch { backtrace, .. }
            | DeserializationError::OffsetOutOfBounds { backtrace, .. }
            | DeserializationError::OffsetSliceOutOfBounds { backtrace, .. }
            | DeserializationError::SerdeFailure { backtrace, .. } => Some(backtrace.clone()),
            DeserializationError::ArrowConvertFailure(_)
            | DeserializationError::DataCellError(_)
            | DeserializationError::ValidationError(_) => None,
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
