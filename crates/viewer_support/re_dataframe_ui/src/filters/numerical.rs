use std::fmt::{Debug, Formatter};

use arrow::array::{Array as _, ArrayRef, BooleanArray};
use arrow::datatypes::{DataType, Field};
use datafusion::common::{Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{Expr, TypeSignature, col, lit, not};
use ordered_float::OrderedFloat;
use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use strum::VariantArray as _;

use super::{Filter, FilterError, FilterUdf, FilterUiAction, action_from_text_edit_response};

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

    pub fn apply<T>(&self, left: T, right: T) -> bool
    where
        T: PartialOrd + PartialEq + Copy,
    {
        match self {
            // Consistent with other column types, we handle `Ne` as an outer-NOT on `Eq`, so the
            // `NOT` is applied in the `as_filter_expression` functions.
            Self::Eq | Self::Ne => left == right,
            Self::Lt => left < right,
            Self::Le => left <= right,
            Self::Gt => left > right,
            Self::Ge => left >= right,
        }
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
}

impl Filter for IntFilter {
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError> {
        let Some(rhs_value) = self.rhs_value else {
            return Ok(lit(true));
        };

        // Consistent with other column types, we treat `Ne` as an outer-NOT with ALL semantics on
        // lists. This is achieved by two things working in concert:
        // - `ComparisonOperator::apply` (which the UDF uses) handles `Ne` identically to `Eq`.
        // - For `Ne`, outer negation is then applied below (see `should_invert_expression`).

        let udf = IntFilterUdf {
            op: self.operator,
            rhs_value,
        }
        .as_scalar_udf();

        let expr = udf.call(vec![col(field.name().clone())]);

        let should_invert_expression = self.operator == ComparisonOperator::Ne;

        Ok(if should_invert_expression {
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
        let operator_text = self.comparison_operator().to_string();

        numerical_comparison_operator_ui(ui, column_name, &operator_text, &mut self.operator);

        let mut value_str = self.rhs_value.map(|v| v.to_string()).unwrap_or_default();
        let response = ui.text_edit_singleline(&mut value_str);
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

/// Wrapper to implement [`FilterUdf`].
///
/// The only purpose of this wrapper is to _not_ have an `Option` around `rhs_value` and thus
/// simplify the implementation.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct IntFilterUdf {
    op: ComparisonOperator,
    rhs_value: i128,
}

impl FilterUdf for IntFilterUdf {
    const PRIMITIVE_SIGNATURE: TypeSignature = TypeSignature::Numeric(1);

    fn name(&self) -> &'static str {
        "int"
    }

    fn is_valid_primitive_input_type(data_type: &DataType) -> bool {
        matches!(data_type, _data_type if data_type.is_integer())
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        macro_rules! int_case {
            ($op_arm:ident, $conv_fun:ident, $slf:expr) => {{
                let array = datafusion::common::cast::$conv_fun(array)?;

                #[allow(clippy::allow_attributes, trivial_numeric_casts)]
                let result: BooleanArray = array
                    .iter()
                    .map(|x| x.map(|x| $slf.op.apply(x, $slf.rhs_value as _)))
                    .collect();

                Ok(result)
            }};
        }

        match array.data_type() {
            DataType::Int8 => int_case!(Int, as_int8_array, self),
            DataType::Int16 => int_case!(Int, as_int16_array, self),
            DataType::Int32 => int_case!(Int, as_int32_array, self),
            DataType::Int64 => int_case!(Int, as_int64_array, self),
            DataType::UInt8 => int_case!(Int, as_uint8_array, self),
            DataType::UInt16 => int_case!(Int, as_uint16_array, self),
            DataType::UInt32 => int_case!(Int, as_uint32_array, self),
            DataType::UInt64 => int_case!(Int, as_uint64_array, self),

            _ => {
                exec_err!("Unsupported data type {}", array.data_type())
            }
        }
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
}

impl Filter for FloatFilter {
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError> {
        let Some(rhs_value) = self.rhs_value else {
            return Ok(lit(true));
        };

        let udf = FloatFilterUdf {
            op: self.operator,
            rhs_value,
        }
        .as_scalar_udf();

        let expr = udf.call(vec![col(field.name().clone())]);

        // Consistent with other column types, we treat `Ne` as an outer-NOT, so we applies it here
        // while the UDF handles `Ne` and `Eq` in the same way (see `ComparisonOperator::apply`).
        let should_invert_expression = self.operator == ComparisonOperator::Ne;

        Ok(if should_invert_expression {
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
        let operator_text = self.comparison_operator().to_string();

        numerical_comparison_operator_ui(ui, column_name, &operator_text, &mut self.operator);

        let mut value_str = self.rhs_value.map(|v| v.to_string()).unwrap_or_default();
        let response = ui.text_edit_singleline(&mut value_str);
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

/// Wrapper to implement [`FilterUdf`].
///
/// The only purpose of this wrapper is to _not_ have an `Option` around `rhs_value` and thus
/// simplify the implementation.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct FloatFilterUdf {
    op: ComparisonOperator,
    rhs_value: OrderedFloat<f64>,
}

impl FilterUdf for FloatFilterUdf {
    const PRIMITIVE_SIGNATURE: TypeSignature = TypeSignature::Numeric(1);

    fn name(&self) -> &'static str {
        "float"
    }

    fn is_valid_primitive_input_type(data_type: &DataType) -> bool {
        match data_type {
            //TODO(ab): float16 support (use `is_floating()`)
            DataType::Float32 | DataType::Float64 => true,

            _ => false,
        }
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        macro_rules! float_case {
            ($op_arm:ident, $conv_fun:ident, $slf:expr) => {{
                let array = datafusion::common::cast::$conv_fun(array)?;

                #[allow(clippy::allow_attributes, trivial_numeric_casts)]
                let result: BooleanArray = array
                    .iter()
                    .map(|x| x.map(|x| $slf.op.apply(x, $slf.rhs_value.into_inner() as _)))
                    .collect();

                Ok(result)
            }};
        }

        match array.data_type() {
            //TODO(ab): float16 support
            DataType::Float32 => float_case!(Float, as_float32_array, self),
            DataType::Float64 => float_case!(Float, as_float64_array, self),

            _ => {
                exec_err!("Unsupported data type {}", array.data_type())
            }
        }
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
            });
    });
}
