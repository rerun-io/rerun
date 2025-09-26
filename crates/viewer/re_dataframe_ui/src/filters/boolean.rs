use arrow::datatypes::{DataType, Field};
use datafusion::common::Column;
use datafusion::logical_expr::{Expr, col, lit};
use datafusion::prelude::{array_element, array_has, array_sort};

use re_ui::UiExt as _;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{FilterError, FilterUiAction};

/// Filter for non-nullable boolean columns.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonNullableBooleanFilter {
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

    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        match field.data_type() {
            DataType::Boolean => Ok(col(column.clone()).eq(lit(self.as_bool()))),

            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Boolean =>
            {
                // `ANY` semantics
                Ok(array_has(col(column.clone()), lit(self.as_bool())))
            }

            _ => Err(FilterError::InvalidNonNullableBooleanFilter(
                self.clone(),
                field.clone().into(),
            )),
        }
    }

    pub fn operand_text(&self) -> String {
        self.as_bool().to_string()
    }

    pub fn popup_ui(&mut self, ui: &mut egui::Ui, column_name: &str) -> FilterUiAction {
        popup_header_ui(ui, column_name);

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

/// Filter for nullable boolean columns.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(clippy::enum_variant_names)]
pub enum NullableBooleanFilter {
    IsTrue,
    IsFalse,
    IsNull,
}

impl NullableBooleanFilter {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::IsTrue => Some(true),
            Self::IsFalse => Some(false),
            Self::IsNull => None,
        }
    }

    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        match field.data_type() {
            DataType::Boolean => {
                if let Some(value) = self.as_bool() {
                    Ok(col(column.clone()).eq(lit(value)))
                } else {
                    Ok(col(column.clone()).is_null())
                }
            }

            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Boolean =>
            {
                // `ANY` semantics
                if let Some(value) = self.as_bool() {
                    Ok(array_has(col(column.clone()), lit(value)))
                } else {
                    Ok(col(column.clone()).is_null().or(array_element(
                        array_sort(col(column.clone()), lit("ASC"), lit("NULLS FIRST")),
                        lit(1),
                    )
                    .is_null()))
                }
            }

            _ => Err(FilterError::InvalidNullableBooleanFilter(
                self.clone(),
                field.clone().into(),
            )),
        }
    }

    pub fn operand_text(&self) -> String {
        if let Some(value) = self.as_bool() {
            value.to_string()
        } else {
            "null".to_owned()
        }
    }

    pub fn popup_ui(&mut self, ui: &mut egui::Ui, column_name: &str) -> FilterUiAction {
        popup_header_ui(ui, column_name);

        let mut clicked = false;

        clicked |= ui
            .re_radio_value(self, Self::IsTrue, primitive_widget_text(ui, "true"))
            .clicked();
        clicked |= ui
            .re_radio_value(self, Self::IsFalse, primitive_widget_text(ui, "false"))
            .clicked();
        clicked |= ui
            .re_radio_value(self, Self::IsNull, primitive_widget_text(ui, "null"))
            .clicked();

        if clicked {
            FilterUiAction::CommitStateToBlueprint
        } else {
            FilterUiAction::None
        }
    }
}

fn primitive_widget_text(ui: &egui::Ui, s: &str) -> egui::WidgetText {
    SyntaxHighlightedBuilder::primitive(s).into_widget_text(ui.style())
}

fn popup_header_ui(ui: &mut egui::Ui, column_name: &str) {
    ui.label(
        SyntaxHighlightedBuilder::body_default(column_name)
            .with_keyword(" is")
            .into_widget_text(ui.style()),
    );
}
