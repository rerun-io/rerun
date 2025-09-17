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
pub struct NonNullableBooleanFilter(pub bool);

impl NonNullableBooleanFilter {
    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        match field.data_type() {
            DataType::Boolean => Ok(col(column.clone()).eq(lit(self.0))),

            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Boolean =>
            {
                // `ANY` semantics""
                Ok(array_has(col(column.clone()), lit(self.0)))
            }

            _ => Err(FilterError::InvalidNonNullableBooleanFilterOperation(
                self.clone(),
                field.clone().into(),
            )),
        }
    }

    pub fn operand_text(&self) -> String {
        self.0.to_string()
    }

    pub fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        column_name: &str,
        action: &mut super::FilterUiAction,
    ) {
        super::basic_operation_ui(ui, column_name, "is");

        let mut clicked = false;

        clicked |= ui
            .re_radio_value(&mut self.0, true, primitive_widget_text(ui, "true"))
            .clicked();

        clicked |= ui
            .re_radio_value(&mut self.0, false, primitive_widget_text(ui, "false"))
            .clicked();

        if clicked {
            *action = FilterUiAction::CommitStateToBlueprint;
        }
    }
}

/// Filter for nullable boolean columns.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullableBooleanFilter(pub Option<bool>); //TODO: make this a full enum

impl NullableBooleanFilter {
    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        match field.data_type() {
            DataType::Boolean => {
                if let Some(value) = self.0 {
                    Ok(col(column.clone()).eq(lit(value)))
                } else {
                    Ok(col(column.clone()).is_null())
                }
            }

            DataType::List(field) | DataType::ListView(field)
                if field.data_type() == &DataType::Boolean =>
            {
                // `ANY` semantics
                if let Some(value) = self.0 {
                    Ok(array_has(col(column.clone()), lit(value)))
                } else {
                    Ok(col(column.clone()).is_null().or(array_element(
                        array_sort(col(column.clone()), lit("ASC"), lit("NULLS FIRST")),
                        lit(1),
                    )
                    .is_null()))
                }
            }

            _ => Err(FilterError::InvalidNullableBooleanFilterOperation(
                self.clone(),
                field.clone().into(),
            )),
        }
    }

    pub fn operand_text(&self) -> String {
        if let Some(value) = self.0 {
            value.to_string()
        } else {
            "null".to_owned()
        }
    }

    pub fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        column_name: &str,
        action: &mut super::FilterUiAction,
    ) {
        super::basic_operation_ui(ui, column_name, "is");

        let mut clicked = false;

        clicked |= ui
            .re_radio_value(&mut self.0, Some(true), primitive_widget_text(ui, "true"))
            .clicked();
        clicked |= ui
            .re_radio_value(&mut self.0, Some(false), primitive_widget_text(ui, "false"))
            .clicked();
        clicked |= ui
            .re_radio_value(&mut self.0, None, primitive_widget_text(ui, "null"))
            .clicked();

        if clicked {
            *action = FilterUiAction::CommitStateToBlueprint;
        }
    }
}

fn primitive_widget_text(ui: &egui::Ui, s: &str) -> egui::WidgetText {
    SyntaxHighlightedBuilder::primitive(s).into_widget_text(ui.style())
}
