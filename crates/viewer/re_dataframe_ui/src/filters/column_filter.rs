use std::fmt::Formatter;

use arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::prelude::Expr;

use super::{Filter as _, FilterError, TypedFilter};

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

/// A filter applied to a table's column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnFilter {
    pub field: FieldRef,
    pub filter: TypedFilter,
}

impl ColumnFilter {
    pub fn new(field: FieldRef, filter: impl Into<TypedFilter>) -> Self {
        Self {
            field,
            filter: filter.into(),
        }
    }

    pub fn default_for_column(field: FieldRef) -> Option<Self> {
        let filter = TypedFilter::default_for_column(&field)?;
        Some(Self::new(field, filter))
    }

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(&self) -> Result<Expr, FilterError> {
        self.filter.as_filter_expression(&self.field)
    }
}
