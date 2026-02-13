use arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::logical_expr::Expr;
use re_log_types::TimestampFormat;
use re_types_core::{Component as _, FIELD_METADATA_KEY_COMPONENT_TYPE, Loggable as _};
use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{
    FilterUiAction, FloatFilter, IntFilter, NonNullableBooleanFilter, Nullability,
    NullableBooleanFilter, StringFilter, TimestampFilter, TimestampFormatted,
    is_supported_string_datatype,
};

#[derive(Debug, Clone, thiserror::Error)]
#[expect(clippy::enum_variant_names)]
pub enum FilterError {
    #[error("invalid non-nullable boolean filter {filter:?} for field {field}")]
    InvalidNonNullableBooleanFilter {
        filter: NonNullableBooleanFilter,
        field: Box<Field>,
    },

    #[error("invalid nullable boolean filter {filter:?} for field {field}")]
    InvalidNullableBooleanFilter {
        filter: NullableBooleanFilter,
        field: Box<Field>,
    },

    #[error("invalid string filter {filter:?} for field {field}")]
    InvalidStringFilter {
        filter: StringFilter,
        field: Box<Field>,
    },
}

/// Trait describing what a filter must do.
pub trait Filter {
    /// Convert the filter to a datafusion expression.
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError>;

    /// Show the UI of the popup associated with this filter.
    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction;

    /// Given a chance to the filter to update/clean itself upon committing the filter state to the
    /// table blueprint.
    ///
    /// This is used e.g. by the timestamp filter to normalize the user entry to the proper
    /// representation of the parsed timestamp.
    fn on_commit(&mut self) {}
}

/// Concrete implementation of a [`Filter`] with static dispatch.
///
/// ## Why does this exists?
///
/// The obvious alternative would be some kind of `Box<dyn FilterTrait>` instead. After trying quite
/// a bit, I decided that the complexity of this is not worth the advantages, which are non-obvious
/// given that all implementations are known and local.
///
/// The complexity of achieving dynamic dispatch stems from filters needing to be:
/// - `Clone` (achievable using the `dyn-clone` crate)
/// - `PartialEq` (which is not dyn-compatible and doesn't have easy work-around)
/// - `TimestampFormatted<T>: SyntaxHighlighting`
///
/// The first two items are required because both `TableBlueprint` and the datafusion machinery
/// need them (e.g., to test blueprint inequality before triggering a costly table update).
/// The last item is related to how the filter UI is implemented using the `SyntaxHighlighting`
/// machinery.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypedFilter {
    NullableBoolean(NullableBooleanFilter),
    NonNullableBoolean(NonNullableBooleanFilter),
    Int(IntFilter),
    Float(FloatFilter),
    String(StringFilter),
    Timestamp(TimestampFilter),
}

impl TypedFilter {
    pub fn default_for_column(column_field: &FieldRef) -> Option<Self> {
        match column_field.data_type() {
            DataType::List(inner_field) | DataType::ListView(inner_field) => {
                // Note: we do not support double-nested types
                Self::default_for_primitive_datatype(inner_field.data_type(), column_field)
            }

            //TODO(ab): support other nested types
            _ => Self::default_for_primitive_datatype(column_field.data_type(), column_field),
        }
    }

    /// Create a [`Self`] instance based on the provided primitive datatype.
    ///
    /// Note that `column_field`, as its name implies, is from the actual column, and its datatype
    /// may be nested (e.g., a list array). This is why `primitive_datatype` is provided as well.
    fn default_for_primitive_datatype(
        primitive_datatype: &DataType,
        column_field: &FieldRef,
    ) -> Option<Self> {
        let nullability = Nullability::from_field(column_field);

        match primitive_datatype {
            DataType::Boolean => {
                if nullability.is_either() {
                    Some(NullableBooleanFilter::default().into())
                } else {
                    Some(NonNullableBooleanFilter::default().into())
                }
            }

            data_type
                if data_type == &re_sdk_types::components::Timestamp::arrow_datatype()
                    && column_field
                        .metadata()
                        .get(FIELD_METADATA_KEY_COMPONENT_TYPE)
                        == Some(&re_sdk_types::components::Timestamp::name().to_string()) =>
            {
                Some(TimestampFilter::default().into())
            }

            data_type if data_type.is_integer() => Some(IntFilter::default().into()),

            DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                Some(FloatFilter::default().into())
            }

            data_type if is_supported_string_datatype(data_type) => {
                Some(StringFilter::default().into())
            }

            DataType::Timestamp(_, _) => Some(TimestampFilter::default().into()),

            _ => None,
        }
    }

    fn as_filter(&self) -> &dyn Filter {
        match self {
            Self::NullableBoolean(inner) => inner,
            Self::NonNullableBoolean(inner) => inner,
            Self::Int(inner) => inner,
            Self::Float(inner) => inner,
            Self::String(inner) => inner,
            Self::Timestamp(inner) => inner,
        }
    }

    fn as_filter_mut(&mut self) -> &mut dyn Filter {
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

impl Filter for TypedFilter {
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

    fn on_commit(&mut self) {
        self.as_filter_mut().on_commit();
    }
}

impl From<NullableBooleanFilter> for TypedFilter {
    fn from(inner: NullableBooleanFilter) -> Self {
        Self::NullableBoolean(inner)
    }
}

impl From<NonNullableBooleanFilter> for TypedFilter {
    fn from(inner: NonNullableBooleanFilter) -> Self {
        Self::NonNullableBoolean(inner)
    }
}

impl From<IntFilter> for TypedFilter {
    fn from(inner: IntFilter) -> Self {
        Self::Int(inner)
    }
}

impl From<FloatFilter> for TypedFilter {
    fn from(inner: FloatFilter) -> Self {
        Self::Float(inner)
    }
}

impl From<StringFilter> for TypedFilter {
    fn from(inner: StringFilter) -> Self {
        Self::String(inner)
    }
}

impl From<TimestampFilter> for TypedFilter {
    fn from(inner: TimestampFilter) -> Self {
        Self::Timestamp(inner)
    }
}

impl SyntaxHighlighting for TimestampFormatted<'_, TypedFilter> {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        match self.inner {
            TypedFilter::NonNullableBoolean(inner) => {
                builder.append(inner);
            }

            TypedFilter::NullableBoolean(inner) => {
                builder.append(inner);
            }

            TypedFilter::Int(inner) => {
                builder.append(inner);
            }

            TypedFilter::Float(inner) => {
                builder.append(inner);
            }

            TypedFilter::String(inner) => {
                builder.append(inner);
            }

            TypedFilter::Timestamp(inner) => {
                builder.append(&self.convert(inner));
            }
        }
    }
}
