use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, BooleanArray, ListArray, as_list_array};
use arrow::datatypes::DataType;
use datafusion::common::{Column, Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{
    ArrayFunctionArgument, ArrayFunctionSignature, ColumnarValue, Expr, ScalarFunctionArgs,
    ScalarUDF, ScalarUDFImpl, Signature, TypeSignature, Volatility, col, lit, not,
};
use strum::VariantArray as _;

use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{FilterUiAction, action_from_text_edit_response};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, strum::VariantArray)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntFilter {
    operator: ComparisonOperator,

    /// The value to compare to.
    ///
    /// `None` means that the user provided no input. In this case, we match everything.
    rhs_value: Option<i128>,
}

impl SyntaxHighlighting for IntFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
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

    pub fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
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

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(&self, column: &Column) -> Expr {
        let Some(rhs_value) = self.rhs_value else {
            return lit(true);
        };

        let udf = ScalarUDF::new_from_impl(IntFilterUdf::new(self.operator, rhs_value));
        let expr = udf.call(vec![col(column.clone())]);

        // Consistent with other column types, we treat `Ne` as an outer-NOT, so we applies it here
        // while the UDF handles `Ne` and `Eq` in the same way (see `ComparisonOperator::apply`).
        let apply_any_or_null_semantics = self.operator == ComparisonOperator::Ne;

        if apply_any_or_null_semantics {
            not(expr.clone()).or(expr.is_null())
        } else {
            expr
        }
    }
}

// ---

/// Filter for floating point column types.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloatFilter {
    operator: ComparisonOperator,

    /// The value to compare to.
    ///
    /// `None` means that the user provided no input. In this case, we match everything.
    rhs_value: Option<f64>,
}

impl SyntaxHighlighting for FloatFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        if let Some(value) = self.rhs_value {
            builder.append_primitive(&re_format::format_f64(value));
        } else {
            builder.append_primitive("…");
        }
    }
}

impl FloatFilter {
    pub fn new(operator: ComparisonOperator, rhs_value: Option<f64>) -> Self {
        Self {
            operator,
            rhs_value,
        }
    }

    pub fn comparison_operator(&self) -> ComparisonOperator {
        self.operator
    }

    pub fn rhs_value(&self) -> Option<f64> {
        self.rhs_value
    }

    pub fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
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

    /// Convert to an [`Expr`].
    ///
    /// The expression is used for filtering and should thus evaluate to a boolean.
    pub fn as_filter_expression(&self, column: &Column) -> Expr {
        let Some(rhs_value) = self.rhs_value else {
            return lit(true);
        };

        let udf = ScalarUDF::new_from_impl(FloatFilterUdf::new(self.operator, rhs_value));

        let expr = udf.call(vec![col(column.clone())]);

        // Consistent with other column types, we treat `Ne` as an outer-NOT, so we applies it here
        // while the UDF handles `Ne` and `Eq` in the same way (see `ComparisonOperator::apply`).
        let apply_any_or_null_semantics = self.operator == ComparisonOperator::Ne;

        if apply_any_or_null_semantics {
            not(expr.clone()).or(expr.is_null())
        } else {
            expr
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

// ---

/// Custom UDF for evaluating some filters kinds.
//TODO(ab): consider splitting the vectorized filtering part from the `any`/`all` aggregation.
#[derive(Debug, Clone)]
struct IntFilterUdf {
    op: ComparisonOperator,
    rhs_value: i128,
    signature: Signature,
}

impl IntFilterUdf {
    fn new(op: ComparisonOperator, rhs_value: i128) -> Self {
        let signature = Signature::one_of(
            vec![
                TypeSignature::Numeric(1),
                TypeSignature::ArraySignature(ArrayFunctionSignature::Array {
                    arguments: vec![ArrayFunctionArgument::Array],
                    array_coercion: None,
                }),
            ],
            Volatility::Immutable,
        );

        Self {
            op,
            rhs_value,
            signature,
        }
    }

    /// Check if the provided _primitive_ type is valid.
    fn is_valid_primitive_input_type(data_type: &DataType) -> bool {
        matches!(data_type, _data_type if data_type.is_integer())
    }

    fn is_valid_input_type(data_type: &DataType) -> bool {
        match data_type {
            DataType::List(field) | DataType::ListView(field) => {
                // Note: we do not support double nested types
                Self::is_valid_primitive_input_type(field.data_type())
            }

            //TODO(ab): support other containers
            _ => Self::is_valid_primitive_input_type(data_type),
        }
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        macro_rules! int_case {
            ($op_arm:ident, $conv_fun:ident, $slf:expr) => {{
                let array = datafusion::common::cast::$conv_fun(array)?;

                #[allow(trivial_numeric_casts)]
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

    fn invoke_list_array(&self, list_array: &ListArray) -> DataFusionResult<BooleanArray> {
        // TODO(ab): we probably should do this in two steps:
        // 1) Convert the list array to a bool array (with same offsets and nulls)
        // 2) Apply the ANY (or, in the future, another) semantics to "merge" each row's instances
        //    into the final bool.
        // TODO(ab): duplicated code with the other UDF, pliz unify.
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

impl ScalarUDFImpl for IntFilterUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "int_filter"
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
                "input data type {} not supported for IntFilter",
                arg_types[0]
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

            _data_type if _data_type.is_integer() => self.invoke_primitive_array(input_array)?,

            _ => {
                return exec_err!(
                    "DataType not implemented for IntFilter: {}",
                    input_array.data_type()
                );
            }
        };

        Ok(ColumnarValue::Array(Arc::new(results)))
    }
}

// ---

/// Custom UDF for evaluating some filters kinds.
//TODO(ab): consider splitting the vectorized filtering part from the `any`/`all` aggregation.
#[derive(Debug, Clone)]
struct FloatFilterUdf {
    op: ComparisonOperator,
    rhs_value: f64,
    signature: Signature,
}

impl FloatFilterUdf {
    fn new(op: ComparisonOperator, rhs_value: f64) -> Self {
        let signature = Signature::one_of(
            vec![
                TypeSignature::Numeric(1),
                TypeSignature::ArraySignature(ArrayFunctionSignature::Array {
                    arguments: vec![ArrayFunctionArgument::Array],
                    array_coercion: None,
                }),
            ],
            Volatility::Immutable,
        );

        Self {
            op,
            rhs_value,
            signature,
        }
    }

    /// Check if the provided _primitive_ type is valid.
    fn is_valid_primitive_input_type(data_type: &DataType) -> bool {
        // TODO(ab): this is technically not correct, and we should distinguish between the i128 and
        // f64 case. Let's deal with this when addressing the redundancy between all the UDFs.
        match data_type {
            //TODO(ab): float16 support (use `is_floating()`)
            DataType::Float32 | DataType::Float64 => true,

            _ => false,
        }
    }

    fn is_valid_input_type(data_type: &DataType) -> bool {
        match data_type {
            DataType::List(field) | DataType::ListView(field) => {
                // Note: we do not support double nested types
                Self::is_valid_primitive_input_type(field.data_type())
            }

            //TODO(ab): support other containers
            _ => Self::is_valid_primitive_input_type(data_type),
        }
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        macro_rules! float_case {
            ($op_arm:ident, $conv_fun:ident, $slf:expr) => {{
                let array = datafusion::common::cast::$conv_fun(array)?;

                #[allow(trivial_numeric_casts)]
                let result: BooleanArray = array
                    .iter()
                    .map(|x| x.map(|x| $slf.op.apply(x, $slf.rhs_value as _)))
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

    fn invoke_list_array(&self, list_array: &ListArray) -> DataFusionResult<BooleanArray> {
        // TODO(ab): we probably should do this in two steps:
        // 1) Convert the list array to a bool array (with same offsets and nulls)
        // 2) Apply the ANY (or, in the future, another) semantics to "merge" each row's instances
        //    into the final bool.
        // TODO(ab): duplicated code with the other UDF, pliz unify.
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

impl ScalarUDFImpl for FloatFilterUdf {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &'static str {
        "float_filter"
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
                "input data type {} not supported for FloatFilter",
                arg_types[0]
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
            DataType::Float32 | DataType::Float64 => self.invoke_primitive_array(input_array)?,

            _ => {
                return exec_err!(
                    "DataType not implemented for FloatFilter: {}",
                    input_array.data_type()
                );
            }
        };

        Ok(ColumnarValue::Array(Arc::new(results)))
    }
}
