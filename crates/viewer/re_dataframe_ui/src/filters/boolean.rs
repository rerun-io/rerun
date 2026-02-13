use std::fmt::Formatter;

use arrow::datatypes::{DataType, Field};
use datafusion::common::Column;
use datafusion::logical_expr::{Expr, col, lit, not};
use datafusion::prelude::{array_element, array_has, array_sort};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{SyntaxHighlighting, UiExt as _};
use strum::VariantArray as _;

use super::{Filter, FilterError, FilterUiAction};

/// Filter for non-nullable boolean columns.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum NonNullableBooleanFilter {
    #[default]
    IsTrue,
    IsFalse,
}

impl NonNullableBooleanFilter {
    pub fn as_bool(&self) -> bool {
        match self {
            Self::IsTrue => true,
            Self::IsFalse => false,
        }
    }
}

impl Filter for NonNullableBooleanFilter {
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError> {
        match field.data_type() {
            DataType::Boolean => Ok(col(field.name().clone()).eq(lit(self.as_bool()))),

            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Boolean =>
            {
                // `ANY` semantics
                Ok(array_has(col(field.name().clone()), lit(self.as_bool())))
            }

            _ => Err(FilterError::InvalidNonNullableBooleanFilter {
                filter: self.clone(),
                field: field.clone().into(),
            }),
        }
    }

    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        _timestamp_format: re_log_types::TimestampFormat,
        column_name: &str,
        _popup_just_opened: bool,
    ) -> FilterUiAction {
        ui.label(
            SyntaxHighlightedBuilder::body_default(column_name)
                .with_keyword(" is")
                .into_widget_text(ui.style()),
        );

        let mut clicked = false;

        clicked |= ui
            .re_radio_value(self, Self::IsTrue, primitive_widget_text(ui, "true"))
            .clicked();

        clicked |= ui
            .re_radio_value(self, Self::IsFalse, primitive_widget_text(ui, "false"))
            .clicked();

        if clicked {
            FilterUiAction::CommitStateToBlueprint
        } else {
            FilterUiAction::None
        }
    }
}

impl SyntaxHighlighting for NonNullableBooleanFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_keyword("is ");
        builder.append_primitive(&self.as_bool().to_string());
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, strum::VariantArray)]
#[expect(clippy::enum_variant_names)]
pub enum NullableBooleanValue {
    #[default]
    IsTrue,
    IsFalse,
    IsNull,
}

impl NullableBooleanValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::IsTrue => Some(true),
            Self::IsFalse => Some(false),
            Self::IsNull => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, strum::VariantArray)]
pub enum NullableBooleanOperator {
    #[default]
    Is,
    IsNot,
}

impl std::fmt::Display for NullableBooleanOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Is => "is".fmt(f),
            Self::IsNot => "is not".fmt(f),
        }
    }
}

/// Filter for nullable boolean columns.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct NullableBooleanFilter {
    value: NullableBooleanValue,
    operator: NullableBooleanOperator,
}

// to make snapshot more compact
impl std::fmt::Debug for NullableBooleanFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let op = match self.operator {
            NullableBooleanOperator::Is => "",
            NullableBooleanOperator::IsNot => "not ",
        };

        f.write_str(&format!("NullableBooleanFilter({op}{:?})", self.value))
    }
}

impl NullableBooleanFilter {
    pub fn new_is_true() -> Self {
        Self {
            value: NullableBooleanValue::IsTrue,
            operator: NullableBooleanOperator::Is,
        }
    }

    pub fn new_is_false() -> Self {
        Self {
            value: NullableBooleanValue::IsFalse,
            operator: NullableBooleanOperator::Is,
        }
    }

    pub fn new_is_null() -> Self {
        Self {
            value: NullableBooleanValue::IsNull,
            operator: NullableBooleanOperator::Is,
        }
    }

    pub fn with_is_not(mut self) -> Self {
        self.operator = NullableBooleanOperator::IsNot;
        self
    }

    pub fn operand_text(&self) -> String {
        if let Some(value) = self.value.as_bool() {
            value.to_string()
        } else {
            "null".to_owned()
        }
    }
}

impl Filter for NullableBooleanFilter {
    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError> {
        let column = Column::from(field.name().clone());

        let expr = match field.data_type() {
            DataType::Boolean => {
                if let Some(value) = self.value.as_bool() {
                    col(column).eq(lit(value))
                } else {
                    col(column).is_null()
                }
            }

            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Boolean =>
            {
                // `ANY` semantics
                if let Some(value) = self.value.as_bool() {
                    array_has(col(column), lit(value))
                } else {
                    col(column.clone()).is_null().or(array_element(
                        array_sort(col(column), lit("ASC"), lit("NULLS FIRST")),
                        lit(1),
                    )
                    .is_null())
                }
            }

            _ => {
                return Err(FilterError::InvalidNullableBooleanFilter {
                    filter: self.clone(),
                    field: field.clone().into(),
                });
            }
        };

        match self.operator {
            NullableBooleanOperator::Is => Ok(expr),
            NullableBooleanOperator::IsNot => Ok(not(expr.clone()).or(expr.is_null())),
        }
    }

    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        _timestamp_format: re_log_types::TimestampFormat,
        column_name: &str,
        _popup_just_opened: bool,
    ) -> FilterUiAction {
        ui.horizontal(|ui| {
            ui.label(
                SyntaxHighlightedBuilder::body_default(column_name).into_widget_text(ui.style()),
            );

            egui::ComboBox::new("null_bool_op", "")
                .selected_text(
                    SyntaxHighlightedBuilder::keyword(&self.operator.to_string())
                        .into_widget_text(ui.style()),
                )
                .show_ui(ui, |ui| {
                    for possible_op in NullableBooleanOperator::VARIANTS {
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

        let mut clicked = false;

        clicked |= ui
            .re_radio_value(
                &mut self.value,
                NullableBooleanValue::IsTrue,
                primitive_widget_text(ui, "true"),
            )
            .clicked();
        clicked |= ui
            .re_radio_value(
                &mut self.value,
                NullableBooleanValue::IsFalse,
                primitive_widget_text(ui, "false"),
            )
            .clicked();
        clicked |= ui
            .re_radio_value(
                &mut self.value,
                NullableBooleanValue::IsNull,
                null_widget_text(ui),
            )
            .clicked();

        if clicked {
            FilterUiAction::CommitStateToBlueprint
        } else {
            FilterUiAction::None
        }
    }
}

impl SyntaxHighlighting for NullableBooleanFilter {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_keyword(&self.operator.to_string());
        builder.append_keyword(" ");
        builder.append_primitive(&self.operand_text());
    }
}

fn null_widget_text(ui: &egui::Ui) -> egui::WidgetText {
    SyntaxHighlightedBuilder::null("null").into_widget_text(ui.style())
}

fn primitive_widget_text(ui: &egui::Ui, s: &str) -> egui::WidgetText {
    SyntaxHighlightedBuilder::primitive(s).into_widget_text(ui.style())
}
