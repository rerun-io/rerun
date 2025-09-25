use std::fmt::Formatter;

use arrow::datatypes::{DataType, Field};
use datafusion::common::Column;
use datafusion::logical_expr::{Expr, col, lit};
use datafusion::prelude::{array_to_string, lower};

use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{FilterError, FilterUiAction, action_from_text_edit_response, basic_operation_ui};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StringOperator {
    #[default]
    Contains,
    //TODO(ab): add more operators
    // BeginWith,
    // EndsWith,
    // Regex,
}

impl std::fmt::Display for StringOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contains => "contains".fmt(f),
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

    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        if self.query.is_empty() {
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
                return Err(FilterError::InvalidStringFilter(
                    self.clone(),
                    field.clone().into(),
                ));
            }
        };

        Ok(contains_patch(
            lower(operand),
            lower(lit(self.query.clone())),
        ))
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

// TODO(ab): this is a workaround for https://github.com/apache/datafusion/pull/16046. Next time we
// update datafusion, this should break compilation. Remove this function and replace
// `contains_patch` by `datafusion::prelude::contains` in the method above.
fn contains_patch(arg1: Expr, arg2: Expr) -> Expr {
    // make sure we break compilation when we update datafusion
    #[cfg(debug_assertions)]
    let _ = datafusion::prelude::contains();

    datafusion::functions::string::contains().call(<[_]>::into_vec(Box::new([arg1, arg2])))
}
