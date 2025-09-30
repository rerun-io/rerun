use std::fmt::Formatter;
use std::sync::Arc;

use arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::prelude::Expr;

use re_log_types::TimestampFormat;
use re_types_core::{Component as _, FIELD_METADATA_KEY_COMPONENT_TYPE};
use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{
    FilterUiAction, FloatFilter, IntFilter, NonNullableBooleanFilter, NullableBooleanFilter,
    StringFilter, TimestampFilter, TimestampFormatted, is_supported_string_datatype,
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

//TODO: use anyhow?
#[derive(Debug, Clone, thiserror::Error)]
pub enum FilterError {
    #[error("invalid non-nullable boolean filter {0:?} for field {1}")]
    InvalidNonNullableBooleanFilter(NonNullableBooleanFilter, Box<Field>),

    #[error("invalid nullable boolean filter {0:?} for field {1}")]
    InvalidNullableBooleanFilter(NullableBooleanFilter, Box<Field>),

    #[error("invalid string filter {0:?} for field {1}")]
    InvalidStringFilter(StringFilter, Box<Field>),
}

/// A filter applied to a table's column.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnFilter {
    pub field: FieldRef,

    // TODO: docstring
    pub filter: Filter,
}

impl ColumnFilter {
    pub fn new(field: FieldRef, filter: impl Into<Filter>) -> Self {
        Self {
            field,
            filter: filter.into(),
        }
    }

    pub fn default_for_column(field: FieldRef) -> Option<Self> {
        match field.data_type() {
            DataType::List(inner_field) | DataType::ListView(inner_field) => {
                // Note: we do not support double-nested types
                Self::default_for_primitive_datatype(Arc::clone(inner_field))
            }

            //TODO(ab): support other nested types
            _ => Self::default_for_primitive_datatype(field),
        }
    }

    fn default_for_primitive_datatype(field: FieldRef) -> Option<Self> {
        let nullability = Nullability::from_field(&field);

        match field.data_type() {
            DataType::Boolean => {
                if nullability.is_either() {
                    Some(Self::new(field, NullableBooleanFilter::default()))
                } else {
                    Some(Self::new(field, NonNullableBooleanFilter::default()))
                }
            }

            DataType::Int64
                if field.metadata().get(FIELD_METADATA_KEY_COMPONENT_TYPE)
                    == Some(&re_types::components::Timestamp::name().to_string()) =>
            {
                Some(Self::new(field, TimestampFilter::default()))
            }

            data_type if data_type.is_integer() => Some(Self::new(field, IntFilter::default())),

            DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                Some(Self::new(field, FloatFilter::default()))
            }

            data_type if is_supported_string_datatype(data_type) => {
                Some(Self::new(field, StringFilter::default()))
            }

            DataType::Timestamp(_, _) => Some(Self::new(field, TimestampFilter::default())),

            _ => None,
        }
    }

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(&self) -> Result<Expr, FilterError> {
        self.filter.as_filter_expression(&self.field)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    NullableBoolean(NullableBooleanFilter),
    NonNullableBoolean(NonNullableBooleanFilter),
    Int(IntFilter),
    Float(FloatFilter),
    String(StringFilter),
    Timestamp(TimestampFilter),
}

impl Filter {
    fn as_filter(&self) -> &dyn FilterTrait {
        match self {
            Self::NullableBoolean(inner) => inner,
            Self::NonNullableBoolean(inner) => inner,
            Self::Int(inner) => inner,
            Self::Float(inner) => inner,
            Self::String(inner) => inner,
            Self::Timestamp(inner) => inner,
        }
    }

    fn as_filter_mut(&mut self) -> &mut dyn FilterTrait {
        match self {
            Self::NullableBoolean(inner) => inner,
            Self::NonNullableBoolean(inner) => inner,
            Self::Int(inner) => inner,
            Self::Float(inner) => inner,
            Self::String(inner) => inner,
            Self::Timestamp(inner) => inner,
        }
    }
}

impl FilterTrait for Filter {
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError> {
        self.as_filter().as_filter_expression(field)
    }

    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction {
        // Reduce the default width unnecessarily expands the popup width (queries as usually vers
        // small).
        ui.spacing_mut().text_edit_width = 150.0;

        self.as_filter_mut()
            .popup_ui(ui, timestamp_format, column_name, popup_just_opened)
    }
}

impl From<NullableBooleanFilter> for Filter {
    fn from(inner: NullableBooleanFilter) -> Self {
        Self::NullableBoolean(inner)
    }
}

impl From<NonNullableBooleanFilter> for Filter {
    fn from(inner: NonNullableBooleanFilter) -> Self {
        Self::NonNullableBoolean(inner)
    }
}

impl From<IntFilter> for Filter {
    fn from(inner: IntFilter) -> Self {
        Self::Int(inner)
    }
}

impl From<FloatFilter> for Filter {
    fn from(inner: FloatFilter) -> Self {
        Self::Float(inner)
    }
}

impl From<StringFilter> for Filter {
    fn from(inner: StringFilter) -> Self {
        Self::String(inner)
    }
}

impl From<TimestampFilter> for Filter {
    fn from(inner: TimestampFilter) -> Self {
        Self::Timestamp(inner)
    }
}

impl SyntaxHighlighting for TimestampFormatted<'_, Filter> {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        match self.inner {
            Filter::NonNullableBoolean(inner) => {
                builder.append(inner);
            }

            Filter::NullableBoolean(inner) => {
                builder.append(inner);
            }

            Filter::Int(inner) => {
                builder.append(inner);
            }

            Filter::Float(inner) => {
                builder.append(inner);
            }

            Filter::String(inner) => {
                builder.append(inner);
            }

            Filter::Timestamp(inner) => {
                builder.append(&self.convert(inner));
            }
        }
    }
}

//TODO: docstrings + move somewhere else?
pub trait FilterTrait {
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError>;

    /// Show the UI of the popup associated with this filter.
    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction;

    /// Given a chance to the underlying filter struct to update/clean itself upon committing the
    /// filter state to the table blueprint.
    ///
    /// This is used e.g. by the timestamp filter to normalize the user entry to the proper
    /// representation of the parsed timestamp.
    fn on_commit(&mut self) {}
}
