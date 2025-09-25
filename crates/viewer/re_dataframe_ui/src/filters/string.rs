use std::any::Any;
use std::fmt::Formatter;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanArray, Datum, LargeStringArray, ListArray, StringArray, StringViewArray,
    as_list_array,
};
use arrow::datatypes::{DataType, Field};
use datafusion::common::{Column, Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ArrayFunctionArgument, ArrayFunctionSignature, ColumnarValue, Expr, ScalarFunctionArgs,
    ScalarUDF, ScalarUDFImpl, Signature, TypeSignature, Volatility, col, lit,
};

use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{FilterUiAction, action_from_text_edit_response, basic_operation_ui};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StringOperator {
    #[default]
    Contains,
    StartsWith,
    EndsWith,
}

impl std::fmt::Display for StringOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contains => "contains".fmt(f),
            Self::StartsWith => "starts with".fmt(f),
            Self::EndsWith => "ends with".fmt(f),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StringFilter {
    operator: StringOperator,
    query: String,
}

impl SyntaxHighlighting for StringFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_string_value(&self.query);
    }
}

impl StringFilter {
    pub fn new(operator: StringOperator, query: impl Into<String>) -> Self {
        Self {
            operator,
            query: query.into(),
        }
    }

    pub fn operator(&self) -> StringOperator {
        self.operator
    }

    pub fn as_filter_expression(&self, column: &Column) -> Expr {
        if self.query.is_empty() {
            return lit(true);
        }

        let udf = ScalarUDF::new_from_impl(StringFilterUdf::new(self));
        udf.call(vec![col(column.clone())])
    }

    pub fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction {
        let operator_text = self.operator.to_string();

        basic_operation_ui(ui, column_name, &operator_text);

        let response = ui.text_edit_singleline(&mut self.query);

        if popup_just_opened {
            response.request_focus();
        }

        action_from_text_edit_response(ui, &response)
    }
}

pub fn is_supported_string_datatype(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Utf8 | DataType::Utf8View | DataType::LargeUtf8
    )
}

/// Custom UDF for performing filtering on a column of strings, with support for list columns.
///
/// This UDF converts both the haystack and the needle to lowercase before performing the queries.
#[derive(Debug, Clone)]
struct StringFilterUdf {
    needle: String,
    operator: StringOperator,
    signature: Signature,
}

impl StringFilterUdf {
    fn new(filter: &StringFilter) -> Self {
        let signature = Signature::one_of(
            vec![
                TypeSignature::String(1),
                TypeSignature::ArraySignature(ArrayFunctionSignature::Array {
                    arguments: vec![ArrayFunctionArgument::Array],
                    array_coercion: None,
                }),
            ],
            Volatility::Immutable,
        );

        Self {
            needle: filter.query.to_lowercase(),
            operator: filter.operator,
            signature,
        }
    }

    fn is_valid_input_type(data_type: &DataType) -> bool {
        match data_type {
            DataType::List(field) | DataType::ListView(field) => {
                // Note: we do not support double nested types
                is_supported_string_datatype(field.data_type())
            }

            //TODO(ab): support other containers
            _ => is_supported_string_datatype(data_type),
        }
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        if !is_supported_string_datatype(array.data_type()) {
            return exec_err!("Unsupported data type {}", array.data_type());
        }

        // We need to convert the haystack to lowercase first. We delegate this task to the existing
        // datafusion `lower` UDF.
        //
        // Note that this _must_ happen here (and, e.g., not at the `Expr` level), because `lower`
        // does not support nested datatypes such as lists.
        let field = Arc::new(Field::new(
            "unused",
            array.data_type().clone(),
            array.is_nullable(),
        ));
        let lowercase_haystack =
            datafusion::functions::string::lower().invoke_with_args(ScalarFunctionArgs {
                args: vec![ColumnarValue::Array(Arc::clone(array))],
                arg_fields: vec![Arc::clone(&field)],
                number_rows: array.len(),
                return_field: field,
            })?;

        let ColumnarValue::Array(haystack_array) = &lowercase_haystack else {
            return exec_err!("Unexpected scalar operand {lowercase_haystack}");
        };

        // make a scalar needle of the right datatype
        let needle: Box<dyn Datum> = match haystack_array.data_type() {
            DataType::Utf8 => Box::new(StringArray::new_scalar(self.needle.clone())),
            DataType::LargeUtf8 => Box::new(LargeStringArray::new_scalar(self.needle.clone())),
            DataType::Utf8View => Box::new(StringViewArray::new_scalar(self.needle.clone())),

            _ => return exec_err!("Unsupported data type {}", haystack_array.data_type()),
        };

        match self.operator {
            StringOperator::Contains => {
                Ok(arrow::compute::contains(haystack_array, needle.as_ref())?)
            }
            StringOperator::StartsWith => Ok(arrow::compute::starts_with(
                haystack_array,
                needle.as_ref(),
            )?),
            StringOperator::EndsWith => {
                Ok(arrow::compute::ends_with(haystack_array, needle.as_ref())?)
            }
        }
    }

    fn invoke_list_array(&self, list_array: &ListArray) -> DataFusionResult<BooleanArray> {
        // TODO(ab): we probably should do this in two steps:
        // 1) Convert the list array to a bool array (with same offsets and nulls)
        // 2) Apply the ANY (or, in the future, another) semantics to "merge" each row's instances
        //    into the final bool.
        list_array
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

impl ScalarUDFImpl for StringFilterUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "string_filter"
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

        if Self::is_valid_input_type(&arg_types[0]) {
            Ok(DataType::Boolean)
        } else {
            exec_err!(
                "input data type {} not supported for string filter",
                arg_types[0]
            )
        }
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> DataFusionResult<ColumnarValue> {
        let ColumnarValue::Array(input_array) = &args.args[0] else {
            return exec_err!("expected array inputs, not scalar values");
        };

        let results = match input_array.data_type() {
            DataType::List(_) | DataType::ListView(_) => {
                let array = as_list_array(input_array);
                self.invoke_list_array(array)?
            }

            data_type if is_supported_string_datatype(data_type) => {
                self.invoke_primitive_array(input_array)?
            }

            _ => {
                return exec_err!(
                    "DataType not implemented for StringFilterUdf: {}",
                    input_array.data_type()
                );
            }
        };

        Ok(ColumnarValue::Array(Arc::new(results)))
    }
}
