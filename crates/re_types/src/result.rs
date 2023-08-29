// NOTE: We have to make an alias, otherwise we'll trigger `thiserror`'s magic codepath which will
// attempt to use nightly features.
pub type _Backtrace = backtrace::Backtrace;

#[derive(thiserror::Error, Debug, Clone)]
pub enum SerializationError {
    #[error("Failed to serialize {location:?}")]
    Context {
        location: String,
        source: Box<SerializationError>,
    },

    #[error("arrow2-convert serialization Failed: {0}")]
    ArrowConvertFailure(String),
}

impl SerializationError {
    /// Returns the _unresolved_ backtrace associated with this error, if it exists.
    ///
    /// Call `resolve()` on the returned [`_Backtrace`] to resolve it (costly!).
    pub fn backtrace(&self) -> Option<_Backtrace> {
        match self {
            SerializationError::Context { .. } | SerializationError::ArrowConvertFailure(_) => None,
        }
    }
}

pub type SerializationResult<T> = ::std::result::Result<T, SerializationError>;

#[derive(thiserror::Error, Debug, Clone)]
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

    #[error("Expected field {field_name:?} to be present in {datatype:#?}")]
    MissingStructField {
        datatype: ::arrow2::datatypes::DataType,
        field_name: String,
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

    #[error("Datacell deserialization Failed: {0}")]
    DataCellError(String),
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
            | DeserializationError::MissingUnionArm { backtrace, .. }
            | DeserializationError::MissingData { backtrace }
            | DeserializationError::DatatypeMismatch { backtrace, .. }
            | DeserializationError::OffsetOutOfBounds { backtrace, .. }
            | DeserializationError::OffsetSliceOutOfBounds { backtrace, .. } => {
                Some(backtrace.clone())
            }
            DeserializationError::ArrowConvertFailure(_)
            | DeserializationError::DataCellError(_) => None,
        }
    }
}

pub type DeserializationResult<T> = ::std::result::Result<T, DeserializationError>;

pub trait ResultExt<T> {
    fn with_context(self, location: impl AsRef<str>) -> Self;
    fn detailed_unwrap(self) -> T;
}

impl<T> ResultExt<T> for SerializationResult<T> {
    #[inline]
    fn with_context(self, location: impl AsRef<str>) -> Self {
        self.map_err(|err| SerializationError::Context {
            location: location.as_ref().into(),
            source: Box::new(err),
        })
    }

    #[track_caller]
    fn detailed_unwrap(self) -> T {
        match self {
            Ok(v) => v,
            Err(err) => {
                let bt = err.backtrace().map(|mut bt| {
                    bt.resolve();
                    bt
                });

                let err = Box::new(err) as Box<dyn std::error::Error>;
                if let Some(bt) = bt {
                    panic!("{}:\n{:#?}", re_error::format(&err), bt)
                } else {
                    panic!("{}", re_error::format(&err))
                }
            }
        }
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

    #[track_caller]
    fn detailed_unwrap(self) -> T {
        match self {
            Ok(v) => v,
            Err(err) => {
                let bt = err.backtrace().map(|mut bt| {
                    bt.resolve();
                    bt
                });

                let err = Box::new(err) as Box<dyn std::error::Error>;
                if let Some(bt) = bt {
                    panic!("{}:\n{:#?}", re_error::format(&err), bt)
                } else {
                    panic!("{}", re_error::format(&err))
                }
            }
        }
    }
}
