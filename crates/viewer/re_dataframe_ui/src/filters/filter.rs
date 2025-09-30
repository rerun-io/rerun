use std::collections::HashMap;
use std::fmt::Formatter;

use arrow::datatypes::{DataType, Field};
use datafusion::common::{DFSchema, ExprSchema as _};
use datafusion::prelude::{Column, Expr};

use re_types_core::{Component as _, FIELD_METADATA_KEY_COMPONENT_TYPE};

use super::{
    FloatFilter, IntFilter, NonNullableBooleanFilter, NullableBooleanFilter, StringFilter,
    TimestampFilter, is_supported_string_datatype,
};

/// The nullability of a nested arrow datatype.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Nullability {
    /// The inner datatype is nullable (e.g. for a list array, one row's array may contain nulls).
    pub inner: bool,

    /// The outer datatype is nullable (e.g, the a list array, one row may have a null instead of an
    /// array).
    pub outer: bool,
}

// for test snapshot naming
impl std::fmt::Debug for Nullability {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (self.inner, self.outer) {
            (false, false) => write!(f, "no_null"),
            (false, true) => write!(f, "outer_null"),
            (true, false) => write!(f, "inner_null"),
            (true, true) => write!(f, "both_null"),
        }
    }
}

impl Nullability {
    pub const NONE: Self = Self {
        inner: false,
        outer: false,
    };

    pub const BOTH: Self = Self {
        inner: true,
        outer: true,
    };

    pub const INNER: Self = Self {
        inner: true,
        outer: false,
    };

    pub const OUTER: Self = Self {
        inner: false,
        outer: true,
    };

    pub const ALL: &'static [Self] = &[Self::NONE, Self::INNER, Self::OUTER, Self::BOTH];

    pub fn from_field(field: &Field) -> Self {
        match field.data_type() {
            DataType::List(inner_field) | DataType::ListView(inner_field) => Self {
                inner: inner_field.is_nullable(),
                outer: field.is_nullable(),
            },

            //TODO(ab): support other containers
            _ => Self {
                inner: field.is_nullable(),
                outer: false,
            },
        }
    }

    pub fn is_either(&self) -> bool {
        self.inner || self.outer
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum FilterError {
    #[error("column {0} was not found")]
    ColumnNotFound(Column),

    #[error("invalid non-nullable boolean filter {0:?} for field {1}")]
    InvalidNonNullableBooleanFilter(NonNullableBooleanFilter, Box<Field>),

    #[error("invalid nullable boolean filter {0:?} for field {1}")]
    InvalidNullableBooleanFilter(NullableBooleanFilter, Box<Field>),

    #[error("invalid string filter {0:?} for field {1}")]
    InvalidStringFilter(StringFilter, Box<Field>),
}

/// A filter applied to a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub column_name: String,
    pub kind: FilterKind,
}

impl Filter {
    pub fn new(column_name: impl Into<String>, kind: FilterKind) -> Self {
        Self {
            column_name: column_name.into(),
            kind,
        }
    }

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(&self, schema: &DFSchema) -> Result<Expr, FilterError> {
        let column = Column::from(self.column_name.clone());
        let Ok(field) = schema.field_from_column(&column) else {
            return Err(FilterError::ColumnNotFound(column));
        };

        self.kind.as_filter_expression(&column, field)
    }
}

/// The UI state for a filter kind.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterKind {
    NullableBoolean(NullableBooleanFilter),
    NonNullableBoolean(NonNullableBooleanFilter),
    Int(IntFilter),
    Float(FloatFilter),
    String(StringFilter),
    Timestamp(TimestampFilter),
}

impl FilterKind {
    /// Create a filter suitable for this column datatype (if any).
    pub fn default_for_column(field: &Field) -> Option<Self> {
        let nullability = Nullability::from_field(field);
        match field.data_type() {
            DataType::List(inner_field) | DataType::ListView(inner_field) => {
                // Note: we do not support double-nested types
                Self::default_for_primitive_datatype(
                    inner_field.data_type(),
                    field.metadata(),
                    nullability,
                )
            }

            //TODO(ab): support other nested types
            _ => Self::default_for_primitive_datatype(
                field.data_type(),
                field.metadata(),
                nullability,
            ),
        }
    }

    fn default_for_primitive_datatype(
        data_type: &DataType,
        metadata: &HashMap<String, String>,
        nullability: Nullability,
    ) -> Option<Self> {
        match data_type {
            DataType::Boolean => {
                if nullability.is_either() {
                    Some(Self::NullableBoolean(Default::default()))
                } else {
                    Some(Self::NonNullableBoolean(Default::default()))
                }
            }

            DataType::Int64
                if metadata.get(FIELD_METADATA_KEY_COMPONENT_TYPE)
                    == Some(&re_types::components::Timestamp::name().to_string()) =>
            {
                Some(Self::Timestamp(TimestampFilter::default()))
            }

            data_type if data_type.is_integer() => Some(Self::Int(Default::default())),

            DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                Some(Self::Float(Default::default()))
            }

            data_type if is_supported_string_datatype(data_type) => {
                Some(Self::String(Default::default()))
            }

            DataType::Timestamp(_, _) => Some(Self::Timestamp(Default::default())),

            _ => None,
        }
    }

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        match self {
            Self::NullableBoolean(boolean_filter) => {
                boolean_filter.as_filter_expression(column, field)
            }
            Self::NonNullableBoolean(boolean_filter) => {
                boolean_filter.as_filter_expression(column, field)
            }
            Self::Int(int_filter) => Ok(int_filter.as_filter_expression(column)),
            Self::Float(float_filter) => Ok(float_filter.as_filter_expression(column)),
            Self::String(string_filter) => Ok(string_filter.as_filter_expression(column)),
            Self::Timestamp(timestamp_filter) => Ok(timestamp_filter.as_filter_expression(column)),
        }
    }
}
