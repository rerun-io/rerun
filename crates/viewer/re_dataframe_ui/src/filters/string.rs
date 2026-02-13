use std::fmt::Formatter;
use std::sync::Arc;

use arrow::array::{ArrayRef, BooleanArray, Datum, LargeStringArray, StringArray, StringViewArray};
use arrow::datatypes::{DataType, Field};
use datafusion::common::{Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ColumnarValue, Expr, ScalarFunctionArgs, TypeSignature, col, lit, not,
};
use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use strum::VariantArray as _;

use super::{Filter, FilterError, FilterUdf, FilterUiAction, action_from_text_edit_response};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, strum::VariantArray)]
pub enum StringOperator {
    #[default]
    Contains,
    DoesNotContain,
    StartsWith,
    EndsWith,
}

impl std::fmt::Display for StringOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contains => "contains".fmt(f),
            Self::DoesNotContain => "does not contain".fmt(f),
            Self::StartsWith => "starts with".fmt(f),
            Self::EndsWith => "ends with".fmt(f),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct StringFilter {
    operator: StringOperator,
    query: String,
}

impl SyntaxHighlighting for StringFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_keyword(&self.operator.to_string());
        builder.append_keyword(" ");
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
}

impl Filter for StringFilter {
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError> {
        if self.query.is_empty() {
            return Ok(lit(true));
        }

        let udf = self.as_scalar_udf();
        let expr = udf.call(vec![col(field.name().clone())]);

        // The udf treats `DoesNotContains` in the same way as `Contains`, so we must apply an
        // outer `NOT` (or null) operation. This way, both operators yield complementary results.
        let apply_should_invert_expression_semantics =
            self.operator == StringOperator::DoesNotContain;

        Ok(if apply_should_invert_expression_semantics {
            not(expr.clone()).or(expr.is_null())
        } else {
            expr
        })
    }

    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        _timestamp_format: re_log_types::TimestampFormat,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction {
        let operator_text = self.operator.to_string();

        ui.horizontal(|ui| {
            ui.label(
                SyntaxHighlightedBuilder::body_default(column_name).into_widget_text(ui.style()),
            );

            egui::ComboBox::new("string_op", "")
                .selected_text(
                    SyntaxHighlightedBuilder::keyword(&operator_text).into_widget_text(ui.style()),
                )
                .show_ui(ui, |ui| {
                    for possible_op in StringOperator::VARIANTS {
                        if ui
                            .button(
                                SyntaxHighlightedBuilder::keyword(&possible_op.to_string())
                                    .into_widget_text(ui.style()),
                            )
                            .clicked()
                        {
                            self.operator = *possible_op;
                        }
                    }
                });
        });

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

impl FilterUdf for StringFilter {
    const PRIMITIVE_SIGNATURE: TypeSignature = TypeSignature::String(1);

    fn name(&self) -> &'static str {
        "string"
    }

    fn is_valid_primitive_input_type(data_type: &DataType) -> bool {
        is_supported_string_datatype(data_type)
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
                config_options: Arc::new(Default::default()),
            })?;

        let ColumnarValue::Array(haystack_array) = &lowercase_haystack else {
            return exec_err!("Unexpected scalar operand {lowercase_haystack}");
        };

        // make a scalar needle of the right datatype
        let lowercase_query = self.query.to_lowercase();
        let needle: Box<dyn Datum> = match haystack_array.data_type() {
            DataType::Utf8 => Box::new(StringArray::new_scalar(lowercase_query.clone())),
            DataType::LargeUtf8 => Box::new(LargeStringArray::new_scalar(lowercase_query.clone())),
            DataType::Utf8View => Box::new(StringViewArray::new_scalar(lowercase_query.clone())),

            _ => return exec_err!("Unsupported data type {}", haystack_array.data_type()),
        };

        match self.operator {
            // Note: reverse ALL-or-none semantics is applied at the expression level.
            StringOperator::Contains | StringOperator::DoesNotContain => {
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
}
