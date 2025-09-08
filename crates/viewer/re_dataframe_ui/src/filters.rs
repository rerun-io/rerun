use arrow::array::{ArrayRef, BooleanArray, ListArray, as_list_array};
use arrow::datatypes::{DataType, Field};
use datafusion::common::cast::as_int64_array;
use datafusion::common::{DFSchema, Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ArrayFunctionArgument, ArrayFunctionSignature, ColumnarValue, ScalarFunctionArgs, ScalarUDF,
    ScalarUDFImpl, Signature, TypeSignature, Volatility,
};
use datafusion::prelude::{
    Column, Expr, array_element, array_has, array_to_string, col, lit, lower,
};
use re_types_core::datatypes::Bool;
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
pub enum FilterError {
    #[error("column {0} was not found")]
    ColumnNotFound(Column),

    #[error("invalid filter operation {0:?} for field {1}")]
    InvalidFilterOperation(FilterOperation, Box<Field>),
}

/// A filter applied to a table.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl ComparisonOperator {
    pub const ALL: &'static [Self] = &[Self::Eq, Self::Ne, Self::Lt, Self::Le, Self::Gt, Self::Ge];

    pub fn operator_text(&self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }

    pub fn apply(&self, expr: Expr, value: impl datafusion::logical_expr::Literal) -> Expr {
        match self {
            Self::Eq => expr.eq(lit(value)),
            Self::Ne => expr.not_eq(lit(value)),
            Self::Lt => expr.lt(lit(value)),
            Self::Le => expr.lt_eq(lit(value)),
            Self::Gt => expr.gt(lit(value)),
            Self::Ge => expr.gt_eq(lit(value)),
        }
    }
}

/// The kind of filter operation
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum FilterOperation {
    /// Compare an integer value to a constant.
    ///
    /// For columns of lists of integers, only the first value is considered.
    IntCompares {
        operator: ComparisonOperator,
        value: i64, //TODO: should these be Option?
    },

    /// Compare a floating point value to a constant.
    ///
    /// For columns of lists of floats, only the first value is considered.
    FloatCompares {
        operator: ComparisonOperator,
        value: f64,
    },

    /// Ch
    //TODO(ab): parameterise that over multiple string ops, e.g. "contains", "starts with", etc.
    StringContains(String),

    BooleanEquals(bool),
}

impl FilterOperation {
    pub fn default_for_datatype(data_type: &DataType) -> Option<Self> {
        match data_type {
            data_type if data_type.is_integer() => Some(Self::IntCompares {
                operator: ComparisonOperator::Eq,
                value: 0,
            }),
            DataType::List(field) | DataType::ListView(field) if field.data_type().is_integer() => {
                Some(Self::IntCompares {
                    operator: ComparisonOperator::Eq,
                    value: 0,
                })
            }

            DataType::Utf8 | DataType::Utf8View => Some(Self::StringContains(String::new())),
            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Utf8
                    || field.data_type() == &DataType::Utf8View =>
            {
                Some(Self::StringContains(String::new()))
            }

            DataType::Boolean => Some(Self::BooleanEquals(true)),
            DataType::List(fields) | DataType::ListView(fields)
                if fields.data_type() == &DataType::Boolean =>
            {
                Some(Self::BooleanEquals(true))
            }

            DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                Some(Self::FloatCompares {
                    operator: ComparisonOperator::Eq,
                    value: 0.0,
                })
            }

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
            Self::IntCompares { operator, value } => {
                let udf = FilterOperationUdf::new(self.clone());
                let udf = ScalarUDF::new_from_impl(udf);

                Ok(udf.call(vec![col(column.clone())]))
            }

            Self::FloatCompares { operator, value } => match field.data_type() {
                data_type if data_type.is_floating() => {
                    Ok(operator.apply(col(column.clone()), *value))
                }

                DataType::List(field) | DataType::ListView(field)
                    if field.data_type().is_floating() =>
                {
                    Ok(operator.apply(array_element(col(column.clone()), lit(0)), *value))
                }

                _ => Err(FilterError::InvalidFilterOperation(
                    self.clone(),
                    field.clone().into(),
                )),
            },

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
                        // for List[Utf8], we concatenate all the instances into a single logical
                        // string
                        array_to_string(col(column.clone()), lit(" "))
                    }

                    _ => {
                        return Err(FilterError::InvalidFilterOperation(
                            self.clone(),
                            field.clone().into(),
                        ));
                    }
                };

                Ok(contains_patch(lower(operand), lower(lit(query_string))))
            }

            Self::BooleanEquals(value) => match field.data_type() {
                DataType::Boolean => Ok(col(column.clone()).eq(lit(*value))),

                DataType::List(field) | DataType::ListView(field)
                    if field.data_type() == &DataType::Boolean =>
                {
                    // all instances must be equal to the filter value
                    Ok(!array_has(col(column.clone()), lit(!*value)))
                }

                _ => Err(FilterError::InvalidFilterOperation(
                    self.clone(),
                    field.clone().into(),
                )),
            },
        }
    }
}

#[derive(Debug, Clone)]
struct FilterOperationUdf {
    op: FilterOperation,
    signature: Signature,
}

impl FilterOperationUdf {
    fn new(op: FilterOperation) -> Self {
        // TODO we can be more explicit here.
        let type_signature = match &op {
            FilterOperation::IntCompares { .. } => TypeSignature::Numeric(1),
            FilterOperation::StringContains(_) => TypeSignature::String(1),
            FilterOperation::BooleanEquals(_) => TypeSignature::Exact(vec![DataType::Boolean]),
            FilterOperation::FloatCompares { .. } => TypeSignature::Numeric(1),
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

    fn is_valid_input_type(&self, data_type: &DataType) -> bool {
        match data_type {
            DataType::List(field) | DataType::ListView(field) => {
                return self.is_valid_input_type(field.data_type());
            }

            _data_type if data_type.is_integer() => {
                if let FilterOperation::IntCompares { .. } = &self.op {
                    return true;
                }
            }

            DataType::Utf8 | DataType::Utf8View | DataType::LargeUtf8 => {
                if let FilterOperation::StringContains(_) = &self.op {
                    return true;
                }
            }

            DataType::Boolean => {
                if let FilterOperation::BooleanEquals(_) = &self.op {
                    return true;
                }
            }

            DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                if let FilterOperation::FloatCompares { .. } = &self.op {
                    return true;
                }
            }

            _ => {}
        }

        false
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        match array.data_type() {
            DataType::Int64 => {
                let FilterOperation::IntCompares { operator, value } = &self.op else {
                    return exec_err!(
                        "Incompatible operation and data types {:?} - {}",
                        self.op,
                        array.data_type()
                    );
                };
                let array = as_int64_array(array)?;
                let result: BooleanArray = array
                    .iter()
                    .map(|x| {
                        x.map(|v| match operator {
                            ComparisonOperator::Eq => &v == value,
                            ComparisonOperator::Ge => &v >= value,
                            ComparisonOperator::Ne => &v != value,
                            ComparisonOperator::Lt => &v < value,
                            ComparisonOperator::Le => &v <= value,
                            ComparisonOperator::Gt => &v > value,
                        })
                    })
                    .collect();

                Ok(result)
            }
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
                    let element_results = self.invoke_primitive_array(&row)?;
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

    fn name(&self) -> &str {
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
            DataType::List(field) => {
                let array = as_list_array(input_array);
                let results = self.invoke_list_array(&array)?;

                Ok(ColumnarValue::Array(Arc::new(results)))
            }
            DataType::Int64 => {
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

// TODO(ab): this is a workaround for https://github.com/apache/datafusion/pull/16046. Next time we
// update datafusion, this should break compilation. Remove this function and replace
// `contains_patch` by `datafusion::prelude::contains` in the method above.
fn contains_patch(arg1: Expr, arg2: Expr) -> Expr {
    // make sure we break compilation when we update datafusion
    #[cfg(debug_assertions)]
    let _ = datafusion::prelude::contains();

    datafusion::functions::string::contains().call(<[_]>::into_vec(Box::new([arg1, arg2])))
}
