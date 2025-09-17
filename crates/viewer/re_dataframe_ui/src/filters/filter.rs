use std::any::Any;
use std::fmt::Formatter;
use std::sync::Arc;

use arrow::array::{ArrayRef, BooleanArray, ListArray, as_list_array};
use arrow::datatypes::{DataType, Field};
use datafusion::common::{DFSchema, ExprSchema as _, Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ArrayFunctionArgument, ArrayFunctionSignature, ColumnarValue, ScalarFunctionArgs, ScalarUDF,
    ScalarUDFImpl, Signature, TypeSignature, Volatility,
};
use datafusion::prelude::{Column, Expr, array_to_string, col, lit, lower};
use ordered_float::OrderedFloat;

use super::BooleanFilter;

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

    #[error("invalid filter operation {0:?} for field {1}")]
    InvalidFilterOperation(FilterOperation, Box<Field>),

    #[error("invalid filter operation {0:?} for field {1}")]
    InvalidBooleanFilterOperation(BooleanFilter, Box<Field>),
}

/// A filter applied to a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub column_name: String,
    pub operation: FilterOperation,
}

impl Filter {
    pub fn new(column_name: impl Into<String>, operation: FilterOperation) -> Self {
        Self {
            column_name: column_name.into(),
            operation,
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

        self.operation.as_filter_expression(&column, field)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl std::fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eq => "==".fmt(f),
            Self::Ne => "!=".fmt(f),
            Self::Lt => "<".fmt(f),
            Self::Le => "<=".fmt(f),
            Self::Gt => ">".fmt(f),
            Self::Ge => ">=".fmt(f),
        }
    }
}

impl ComparisonOperator {
    pub const ALL: &'static [Self] = &[Self::Eq, Self::Ne, Self::Lt, Self::Le, Self::Gt, Self::Ge];

    pub fn as_ascii(&self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Gt => "gt",
            Self::Ge => "ge",
        }
    }

    pub fn apply<T>(&self, left: T, right: T) -> bool
    where
        T: PartialOrd + PartialEq + Copy,
    {
        match self {
            Self::Eq => left == right,
            Self::Ne => left != right,
            Self::Lt => left < right,
            Self::Le => left <= right,
            Self::Gt => left > right,
            Self::Ge => left >= right,
        }
    }
}

/// The kind of filter operation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FilterOperation {
    /// Compare an integer value to a constant.
    ///
    /// For columns of lists of integers, only the first value is considered.
    IntCompares {
        operator: ComparisonOperator,
        value: Option<i128>,
    },

    /// Compare a floating point value to a constant.
    ///
    /// For columns of lists of floats, only the first value is considered.
    FloatCompares {
        operator: ComparisonOperator,
        value: Option<OrderedFloat<f64>>,
    },

    //TODO(ab): parameterise that over multiple string ops, e.g. "contains", "starts with", etc.
    StringContains(String),

    Boolean(BooleanFilter),
}

impl FilterOperation {
    fn default_for_primitive_datatype(
        data_type: &DataType,
        nullability: Nullability,
    ) -> Option<Self> {
        match data_type {
            data_type if data_type.is_integer() => Some(Self::IntCompares {
                operator: ComparisonOperator::Eq,
                value: None,
            }),

            DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                Some(Self::FloatCompares {
                    operator: ComparisonOperator::Eq,
                    value: None,
                })
            }

            DataType::Utf8 | DataType::Utf8View => Some(Self::StringContains(String::new())),

            DataType::Boolean => Some(Self::Boolean(BooleanFilter::default_for_nullability(
                nullability,
            ))),

            _ => None,
        }
    }

    pub fn default_for_column(field: &Field) -> Option<Self> {
        let nullability = Nullability::from_field(field);
        match field.data_type() {
            DataType::List(inner_field) | DataType::ListView(inner_field) => {
                // Note: we do not support double-nested types
                Self::default_for_primitive_datatype(inner_field.data_type(), nullability)
            }

            //TODO(ab): support other nested types
            _ => Self::default_for_primitive_datatype(field.data_type(), nullability),
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
            Self::IntCompares { .. } | Self::FloatCompares { .. } => {
                let udf = FilterOperationUdf::new(self.clone());
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
                        return Err(FilterError::InvalidFilterOperation(
                            self.clone(),
                            field.clone().into(),
                        ));
                    }
                };

                Ok(datafusion::prelude::contains(
                    lower(operand),
                    lower(lit(query_string)),
                ))
            }

            Self::Boolean(boolean_filter) => boolean_filter.as_filter_expression(column, field),
        }
    }
}

/// Custom UDF for evaluating filter operations.
//TODO(ab): consider splitting the vectorized filtering part from the `any`/`all` aggregation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FilterOperationUdf {
    op: FilterOperation,
    signature: Signature,
}

impl FilterOperationUdf {
    fn new(op: FilterOperation) -> Self {
        debug_assert!(matches!(
            op,
            FilterOperation::IntCompares { .. } | FilterOperation::FloatCompares { .. }
        ));

        let type_signature = TypeSignature::Numeric(1);

        // TODO(ab): add support for other filter types?
        // let type_signature = match &op {
        //     FilterOperation::IntCompares { .. } | FilterOperation::FloatCompares { .. } => {
        //         TypeSignature::Numeric(1)
        //     }
        //     FilterOperation::StringContains(_) => TypeSignature::String(1),
        //     FilterOperation::BooleanEquals(_) => TypeSignature::Exact(vec![DataType::Boolean]),
        // };

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
            _data_type if data_type.is_integer() => {
                matches!(&self.op, FilterOperation::IntCompares { .. })
            }

            //TODO(ab): float16 support (use `is_floating()`)
            DataType::Float32 | DataType::Float64 => {
                matches!(&self.op, FilterOperation::FloatCompares { .. })
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
                let FilterOperation::$op_arm { operator, value } = &$op else {
                    return exec_err!(
                        "Incompatible operation and data types {:?} - {}",
                        $op,
                        array.data_type()
                    );
                };
                let array = datafusion::common::cast::$conv_fun(array)?;
                #[allow(trivial_numeric_casts)]
                let result: BooleanArray = array
                    .iter()
                    .map(|x| {
                        // accept everything if the value is not set
                        let Some(value) = value else {
                            return Some(true);
                        };
                        use num_traits::AsPrimitive as _;
                        x.map(|x| operator.apply(x, value.as_()))
                    })
                    .collect();

                Ok(result)
            }};
        }

        match array.data_type() {
            DataType::Int8 => int_float_case!(IntCompares, as_int8_array, self.op),
            DataType::Int16 => int_float_case!(IntCompares, as_int16_array, self.op),
            DataType::Int32 => int_float_case!(IntCompares, as_int32_array, self.op),
            DataType::Int64 => int_float_case!(IntCompares, as_int64_array, self.op),
            DataType::UInt8 => int_float_case!(IntCompares, as_uint8_array, self.op),
            DataType::UInt16 => int_float_case!(IntCompares, as_uint16_array, self.op),
            DataType::UInt32 => int_float_case!(IntCompares, as_uint32_array, self.op),
            DataType::UInt64 => int_float_case!(IntCompares, as_uint64_array, self.op),

            //TODO(ab): float16 support
            DataType::Float32 => int_float_case!(FloatCompares, as_float32_array, self.op),
            DataType::Float64 => int_float_case!(FloatCompares, as_float64_array, self.op),

            _ => {
                exec_err!("Unsupported data type {}", array.data_type())
            }
        }
    }

    fn invoke_list_array(&self, array: &ListArray) -> DataFusionResult<BooleanArray> {
        array
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

impl ScalarUDFImpl for FilterOperationUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "filter_operation"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, arg_types: &[DataType]) -> DataFusionResult<DataType> {
        if arg_types.len() != 1 {
            return exec_err!(
                "FilterOperation expected a single column of input, received {}",
                arg_types.len()
            );
        }

        if self.is_valid_input_type(&arg_types[0]) {
            Ok(DataType::Boolean)
        } else {
            exec_err!(
                "FilterOperation input data type {} not supported for operation {:?}",
                arg_types[0],
                self.op
            )
        }
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> DataFusionResult<ColumnarValue> {
        let ColumnarValue::Array(input_array) = &args.args[0] else {
            return exec_err!("FilterOperation expected array inputs, not scalar values");
        };
        match input_array.data_type() {
            DataType::List(_field) => {
                let array = as_list_array(input_array);
                let results = self.invoke_list_array(array)?;

                Ok(ColumnarValue::Array(Arc::new(results)))
            }

            //TODO(ab): float16 support (use `is_floating()`)
            DataType::Float32 | DataType::Float64 => {
                let results = self.invoke_primitive_array(input_array)?;
                Ok(ColumnarValue::Array(Arc::new(results)))
            }

            _data_type if _data_type.is_integer() => {
                let results = self.invoke_primitive_array(input_array)?;
                Ok(ColumnarValue::Array(Arc::new(results)))
            }

            _ => {
                exec_err!(
                    "DataType not implemented for FilterOperationUdf: {}",
                    input_array.data_type()
                )
            }
        }
    }
}
