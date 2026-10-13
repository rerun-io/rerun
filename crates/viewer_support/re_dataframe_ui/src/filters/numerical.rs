use std::fmt::{Debug, Formatter};

use arrow::datatypes::DataType;
use datafusion::sql::sqlparser::ast::{BinaryOperator, Expr as SqlExpr};
use ordered_float::OrderedFloat;
use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use strum::VariantArray as _;

use super::{
    Filter, FilterUiAction, SqlColumn, action_from_text_edit_response, negated_sql, number_literal,
    quote_string, string_literal, strip_negation, strip_nested,
};

#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, strum::VariantArray)]
pub enum ComparisonOperator {
    #[default]
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
            Self::Eq => f.write_str("=="),
            Self::Ne => f.write_str("!="),
            Self::Lt => f.write_str("<"),
            Self::Le => f.write_str("<="),
            Self::Gt => f.write_str(">"),
            Self::Ge => f.write_str(">="),
        }
    }
}

impl ComparisonOperator {
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

    /// The SQL comparing a column to `value`.
    fn to_sql(self, column: &SqlColumn<'_>, value: &str) -> String {
        let sql_operator = match self {
            Self::Ne => {
                return negated_sql(&Self::Eq.to_sql(column, value));
            }
            Self::Eq => "=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        };

        column.predicate_sql(|column| format!("{column} {sql_operator} {value}"))
    }

    /// Parse the SQL written by [`Self::to_sql`], returning the operator and the value.
    fn from_sql<'e>(expr: &'e SqlExpr, column: &SqlColumn<'_>) -> Option<(Self, &'e SqlExpr)> {
        let (expr, negated) = strip_negation(expr);
        let (predicate, value) = column.match_predicate(expr)?;
        let (op, right) = value.comparison(predicate)?;

        let operator = match (op, negated) {
            (BinaryOperator::Eq, false) => Self::Eq,
            (BinaryOperator::Eq, true) => Self::Ne,
            (BinaryOperator::Lt, false) => Self::Lt,
            (BinaryOperator::LtEq, false) => Self::Le,
            (BinaryOperator::Gt, false) => Self::Gt,
            (BinaryOperator::GtEq, false) => Self::Ge,
            _ => return None,
        };

        Some((operator, right))
    }
}

// ---

/// Filter for integer column types.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct IntFilter {
    operator: ComparisonOperator,

    /// The value to compare to.
    ///
    /// `None` means that the user provided no input. In this case, we match everything.
    rhs_value: Option<i128>,
}

impl SyntaxHighlighting for IntFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_keyword(&self.comparison_operator().to_string());
        builder.append_keyword(" ");

        if let Some(value) = self.rhs_value {
            builder.append_primitive(&re_format::format_int(value));
        } else {
            builder.append_primitive("…");
        }
    }
}

impl IntFilter {
    pub fn new(operator: ComparisonOperator, rhs_value: Option<i128>) -> Self {
        Self {
            operator,
            rhs_value,
        }
    }

    pub fn comparison_operator(&self) -> ComparisonOperator {
        self.operator
    }

    pub fn rhs_value(&self) -> Option<i128> {
        self.rhs_value
    }

    /// Parse the SQL written by `Filter::to_sql`.
    pub fn from_sql(expr: &SqlExpr, column: &SqlColumn<'_>) -> Option<Self> {
        let (operator, value) = ComparisonOperator::from_sql(expr, column)?;
        Some(Self::new(
            operator,
            Some(number_literal(value)?.parse().ok()?),
        ))
    }
}

impl Filter for IntFilter {
    fn to_sql(&self, column: &SqlColumn<'_>) -> Option<String> {
        let value = self.rhs_value?;
        Some(self.operator.to_sql(column, &value.to_string()))
    }

    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        _timestamp_format: re_log_types::TimestampFormat,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction {
        let operator_text = self.comparison_operator().to_string();

        numerical_comparison_operator_ui(ui, column_name, &operator_text, &mut self.operator);

        let mut value_str = self.rhs_value.map(|v| v.to_string()).unwrap_or_default();
        let response = ui
            .text_edit_singleline(&mut value_str)
            .accessible_name("Value");
        if response.changed() {
            if value_str.is_empty() {
                self.rhs_value = None;
            } else if let Ok(parsed) = value_str.parse() {
                self.rhs_value = Some(parsed);
            }
        }

        if popup_just_opened {
            response.request_focus();
        }

        action_from_text_edit_response(ui, &response)
    }
}

// ---

/// Filter for floating point column types.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FloatFilter {
    operator: ComparisonOperator,

    /// The value to compare to.
    ///
    /// `None` means that the user provided no input. In this case, we match everything.
    rhs_value: Option<OrderedFloat<f64>>,
}

impl SyntaxHighlighting for FloatFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_keyword(&self.comparison_operator().to_string());
        builder.append_keyword(" ");

        if let Some(value) = self.rhs_value {
            builder.append_primitive(&re_format::format_f64(value.into_inner()));
        } else {
            builder.append_primitive("…");
        }
    }
}

impl FloatFilter {
    pub fn new(operator: ComparisonOperator, rhs_value: Option<f64>) -> Self {
        Self {
            operator,
            rhs_value: rhs_value.map(OrderedFloat),
        }
    }

    pub fn comparison_operator(&self) -> ComparisonOperator {
        self.operator
    }

    pub fn rhs_value(&self) -> Option<f64> {
        self.rhs_value.map(|x| x.into_inner())
    }

    /// Parse the SQL written by `Filter::to_sql`.
    pub fn from_sql(expr: &SqlExpr, column: &SqlColumn<'_>) -> Option<Self> {
        let (operator, value) = ComparisonOperator::from_sql(expr, column)?;
        let value = match strip_nested(value) {
            SqlExpr::Cast { expr, .. } => match string_literal(expr) {
                Some(value) => value.parse().ok()?,
                None => number_literal(expr)?.parse().ok()?,
            },
            value => number_literal(value)?.parse().ok()?,
        };
        Some(Self::new(operator, Some(value)))
    }
}

impl Filter for FloatFilter {
    fn to_sql(&self, column: &SqlColumn<'_>) -> Option<String> {
        let value = self.rhs_value?.into_inner();

        // The value has the precision of a `Float32` column, so `0.1` matches the values of
        // such a column that display as `0.1`.
        let is_float32 = column.value_type() == &DataType::Float32;

        // SQL has no literals for infinity and NaN.
        let value_sql = match (value.is_finite(), is_float32) {
            (true, false) => format!("{value:?}"),
            (true, true) => format!("CAST({value:?} AS REAL)"),
            (false, false) => format!("CAST({} AS DOUBLE)", quote_string(&value.to_string())),
            (false, true) => format!("CAST({} AS REAL)", quote_string(&value.to_string())),
        };

        Some(self.operator.to_sql(column, &value_sql))
    }

    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        _timestamp_format: re_log_types::TimestampFormat,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction {
        let operator_text = self.comparison_operator().to_string();

        numerical_comparison_operator_ui(ui, column_name, &operator_text, &mut self.operator);

        let mut value_str = self.rhs_value.map(|v| v.to_string()).unwrap_or_default();
        let response = ui
            .text_edit_singleline(&mut value_str)
            .accessible_name("Value");
        if response.changed() {
            if value_str.is_empty() {
                self.rhs_value = None;
            } else if let Ok(parsed) = value_str.parse() {
                self.rhs_value = Some(parsed);
            }
        }

        if popup_just_opened {
            response.request_focus();
        }

        action_from_text_edit_response(ui, &response)
    }
}

// ---

fn numerical_comparison_operator_ui(
    ui: &mut egui::Ui,
    column_name: &str,
    operator_text: &str,
    op: &mut ComparisonOperator,
) {
    ui.horizontal(|ui| {
        ui.label(SyntaxHighlightedBuilder::body_default(column_name).into_widget_text(ui.style()));

        egui::ComboBox::new("comp_op", "")
            .selected_text(
                SyntaxHighlightedBuilder::keyword(operator_text).into_widget_text(ui.style()),
            )
            .show_ui(ui, |ui| {
                for possible_op in crate::filters::ComparisonOperator::VARIANTS {
                    if ui
                        .button(
                            SyntaxHighlightedBuilder::keyword(&possible_op.to_string())
                                .into_widget_text(ui.style()),
                        )
                        .clicked()
                    {
                        *op = *possible_op;
                    }
                }
            })
            .response
            .accessible_name("Operator");
    });
}
