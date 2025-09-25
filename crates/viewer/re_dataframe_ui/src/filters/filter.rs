use std::any::Any;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, AsArray as _, BooleanArray, ListArray, as_list_array};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field, TimeUnit};
use datafusion::common::ExprSchema as _;
use datafusion::common::{DFSchema, Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ArrayFunctionArgument, ArrayFunctionSignature, ColumnarValue, ScalarFunctionArgs, ScalarUDF,
    ScalarUDFImpl, Signature, TypeSignature, Volatility,
};
use datafusion::prelude::{Column, Expr, array_to_string, col, contains, lit, lower};

use re_types_core::datatypes::TimeInt;
use re_types_core::{Component as _, FIELD_METADATA_KEY_COMPONENT_TYPE, Loggable as _};

use super::{
    FloatFilter, IntFilter, NonNullableBooleanFilter, NullableBooleanFilter, TimestampFilter,
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

    #[error("invalid filter kind {0:?} for field {1}")]
    InvalidFilterKind(Box<FilterKind>, Box<Field>),

    #[error("invalid non-nullable boolean filter {0:?} for field {1}")]
    InvalidNonNullableBooleanFilter(NonNullableBooleanFilter, Box<Field>),

    #[error("invalid nullable boolean filter {0:?} for field {1}")]
    InvalidNullableBooleanFilter(NullableBooleanFilter, Box<Field>),
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

    //TODO(ab): parameterise that over multiple string ops, e.g. "contains", "starts with", etc.
    StringContains(String),

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
                    Some(Self::NullableBoolean(NullableBooleanFilter::IsTrue))
                } else {
                    Some(Self::NonNullableBoolean(NonNullableBooleanFilter::IsTrue))
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

            DataType::Utf8 | DataType::Utf8View => Some(Self::StringContains(String::new())),

            DataType::Timestamp(_, _) => Some(Self::Timestamp(TimestampFilter::default())),

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

            Self::Int(_) | Self::Float(_) | Self::Timestamp(_) => {
                let udf = FilterKindUdf::new(self.clone());
                let udf = ScalarUDF::new_from_impl(udf);

                Ok(udf.call(vec![col(column.clone())]))
            }

            Self::StringContains(query_string) => {
                if query_string.is_empty() {
                    return Ok(lit(true));
                }

                let operand = match field.data_type() {
                    DataType::Utf8 | DataType::Utf8View => col(column.clone()),

                    DataType::List(field) | DataType::ListView(field)
                        if field.data_type() == &DataType::Utf8
                            || field.data_type() == &DataType::Utf8View =>
                    {
                        // For List[Utf8], we concatenate all the instances into a single logical
                        // string, separated by a Record Separator (0x1E) character. This ensures
                        // that the query string doesn't accidentally match a substring spanning
                        // multiple instances.
                        array_to_string(col(column.clone()), lit("\u{001E}"))
                    }

                    _ => {
                        return Err(FilterError::InvalidFilterKind(
                            self.clone().into(),
                            field.clone().into(),
                        ));
                    }
                };

                Ok(contains(lower(operand), lower(lit(query_string))))
            }
        }
    }
}

/// Custom UDF for evaluating some filters kinds.
//TODO(ab): consider splitting the vectorized filtering part from the `any`/`all` aggregation.
#[derive(Debug, Clone)]
struct FilterKindUdf {
    op: FilterKind,
    signature: Signature,
}

impl FilterKindUdf {
    fn new(op: FilterKind) -> Self {
        let type_signature = match op {
            FilterKind::Int(_) | FilterKind::Float(_) => TypeSignature::Numeric(1),

            FilterKind::Timestamp(_) => TypeSignature::Any(1),

            // TODO(ab): add support for other filter types?
            // FilterKind::StringContains(_) => TypeSignature::String(1),
            // FilterKind::BooleanEquals(_) => TypeSignature::Exact(vec![DataType::Boolean]),
            _ => {
                debug_assert!(false, "Invalid filter kind");
                TypeSignature::Any(1)
            }
        };

        let signature = Signature::one_of(
            vec![
                type_signature,
                TypeSignature::ArraySignature(ArrayFunctionSignature::Array {
                    arguments: vec![ArrayFunctionArgument::Array],
                    array_coercion: None,
                }),
            ],
            Volatility::Immutable,
        );

        Self { op, signature }
    }

    /// Check if the provided _primitive_ type is valid.
    fn is_valid_primitive_input_type(&self, data_type: &DataType) -> bool {
        match data_type {
            _data_type if _data_type == &TimeInt::arrow_datatype() => {
                // TimeInt special case: we allow filtering by timestamp on Int64 columns
                matches!(&self.op, FilterKind::Int(_) | FilterKind::Timestamp(_))
            }

            _data_type if data_type.is_integer() => {
                matches!(&self.op, FilterKind::Int(_))
            }

            //TODO(ab): float16 support (use `is_floating()`)
            DataType::Float32 | DataType::Float64 => {
                matches!(&self.op, FilterKind::Float(_))
            }

            DataType::Timestamp(_, _) => {
                matches!(&self.op, FilterKind::Timestamp(_))
            }

            _ => false,
        }
    }

    fn is_valid_input_type(&self, data_type: &DataType) -> bool {
        match data_type {
            DataType::List(field) | DataType::ListView(field) => {
                // Note: we do not support double nested types
                self.is_valid_primitive_input_type(field.data_type())
            }

            //TODO(ab): support other containers
            _ => self.is_valid_primitive_input_type(data_type),
        }
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        macro_rules! int_float_case {
            ($op_arm:ident, $conv_fun:ident, $op:expr) => {{
                let FilterKind::$op_arm(filter) = &$op else {
                    return exec_err!(
                        "Incompatible filter kind and data types {:?} - {}",
                        $op,
                        array.data_type()
                    );
                };
                let array = datafusion::common::cast::$conv_fun(array)?;

                #[allow(trivial_numeric_casts)]
                let result: BooleanArray = array
                    .iter()
                    .map(|x| {
                        let Some(rhs_value) = filter.rhs_value() else {
                            return Some(true);
                        };

                        x.map(|x| filter.comparison_operator().apply(x, rhs_value as _))
                    })
                    .collect();

                Ok(result)
            }};
        }

        macro_rules! timestamp_case {
            ($apply_fun:ident, $conv_fun:ident, $op:expr) => {{
                let FilterKind::Timestamp(timestamp_filter) = &$op else {
                    return exec_err!(
                        "Incompatible filter and data types {:?} - {}",
                        $op,
                        array.data_type()
                    );
                };
                let array = datafusion::common::cast::$conv_fun(array)?;
                let result: BooleanArray = array
                    .iter()
                    .map(|x| x.map(|v| timestamp_filter.$apply_fun(v)))
                    .collect();

                Ok(result)
            }};
        }

        match array.data_type() {
            DataType::Int8 => int_float_case!(Int, as_int8_array, self.op),
            DataType::Int16 => int_float_case!(Int, as_int16_array, self.op),
            DataType::Int32 => int_float_case!(Int, as_int32_array, self.op),

            // Note: although `TimeInt` is Int64, by now we casted it to `Timestamp`, see
            // `invoke_list_array` impl.
            DataType::Int64 => int_float_case!(Int, as_int64_array, self.op),
            DataType::UInt8 => int_float_case!(Int, as_uint8_array, self.op),
            DataType::UInt16 => int_float_case!(Int, as_uint16_array, self.op),
            DataType::UInt32 => int_float_case!(Int, as_uint32_array, self.op),
            DataType::UInt64 => int_float_case!(Int, as_uint64_array, self.op),

            //TODO(ab): float16 support
            DataType::Float32 => int_float_case!(Float, as_float32_array, self.op),
            DataType::Float64 => int_float_case!(Float, as_float64_array, self.op),

            DataType::Timestamp(TimeUnit::Second, _) => {
                timestamp_case!(apply_seconds, as_timestamp_second_array, self.op)
            }
            DataType::Timestamp(TimeUnit::Millisecond, _) => {
                timestamp_case!(apply_milliseconds, as_timestamp_millisecond_array, self.op)
            }
            DataType::Timestamp(TimeUnit::Microsecond, _) => {
                timestamp_case!(apply_microseconds, as_timestamp_microsecond_array, self.op)
            }
            DataType::Timestamp(TimeUnit::Nanosecond, _) => {
                timestamp_case!(apply_nanoseconds, as_timestamp_nanosecond_array, self.op)
            }

            _ => {
                exec_err!("Unsupported data type {}", array.data_type())
            }
        }
    }

    fn invoke_list_array(&self, list_array: &ListArray) -> DataFusionResult<BooleanArray> {
        // TimeInt special case: we cast the Int64 array TimestampNano
        let cast_list_array = if list_array.values().data_type() == &TimeInt::arrow_datatype()
            && matches!(self.op, FilterKind::Timestamp(_))
        {
            let DataType::List(field) = list_array.data_type() else {
                unreachable!("ListArray must have a List data type");
            };
            let new_field = Arc::new(Arc::unwrap_or_clone(field.clone()).with_data_type(
                DataType::Timestamp(TimeUnit::Nanosecond, Some("+00:00".into())),
            ));

            Some(cast(list_array, &DataType::List(new_field))?)
        } else {
            None
        };

        let cast_list_array = cast_list_array
            .as_ref()
            .map(|array| array.as_list())
            .unwrap_or(list_array);

        // TODO(ab): we probably should do this in two steps:
        // 1) Convert the list array to a bool array (with same offsets and nulls)
        // 2) Apply the ANY (or, in the future, another) semantics to "merge" each row's instances
        //    into the final bool.
        cast_list_array
            .iter()
            .map(|maybe_row| {
                maybe_row.map(|row| {
                    // Note: we know this is a primitive array because we explicitly disallow nested
                    // lists or other containers.
                    let element_results = self.invoke_primitive_array(&row)?;

                    // `ANY` semantics happening here
                    Ok(element_results
                        .iter()
                        .map(|x| x.unwrap_or(false))
                        .find(|x| *x)
                        .unwrap_or(false))
                })
            })
            .map(|x| x.transpose())
            .collect::<DataFusionResult<BooleanArray>>()
    }
}

impl ScalarUDFImpl for FilterKindUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "filter_kind"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, arg_types: &[DataType]) -> DataFusionResult<DataType> {
        if arg_types.len() != 1 {
            return exec_err!(
                "expected a single column of input, received {}",
                arg_types.len()
            );
        }

        if self.is_valid_input_type(&arg_types[0]) {
            Ok(DataType::Boolean)
        } else {
            exec_err!(
                "input data type {} not supported for filter {:?}",
                arg_types[0],
                self.op
            )
        }
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> DataFusionResult<ColumnarValue> {
        let ColumnarValue::Array(input_array) = &args.args[0] else {
            return exec_err!("expected array inputs, not scalar values");
        };

        let results = match input_array.data_type() {
            DataType::List(_field) => {
                let array = as_list_array(input_array);
                self.invoke_list_array(array)?
            }

            //TODO(ab): float16 support (use `is_floating()`)
            DataType::Float32 | DataType::Float64 | DataType::Timestamp(_, _) => {
                self.invoke_primitive_array(input_array)?
            }

            _data_type if _data_type.is_integer() => self.invoke_primitive_array(input_array)?,

            _ => {
                return exec_err!(
                    "DataType not implemented for FilterKindUdf: {}",
                    input_array.data_type()
                );
            }
        };

        Ok(ColumnarValue::Array(Arc::new(results)))
    }
}
