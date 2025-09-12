use arrow::datatypes::{DataType, Field};
use datafusion::common::Column;
use datafusion::logical_expr::{Expr, col, lit};
use datafusion::prelude::{array_element, array_has, array_sort};

use re_ui::UiExt as _;

use super::{FilterError, FilterUiAction, Nullability};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanFilter {
    NonNullable(bool),
    Nullable(Option<bool>),
}

impl BooleanFilter {
    pub fn default_for_nullability(nullability: Nullability) -> Self {
        if nullability.is_either() {
            Self::Nullable(Some(true))
        } else {
            Self::NonNullable(true)
        }
    }

    pub fn as_filter_expression(
        &self,
        column: &Column,
        field: &Field,
    ) -> Result<Expr, FilterError> {
        match self {
            Self::NonNullable(value) => match field.data_type() {
                DataType::Boolean => Ok(col(column.clone()).eq(lit(*value))),

                DataType::List(field) | DataType::ListView(field)
                    if field.data_type() == &DataType::Boolean =>
                {
                    // `ANY` semantics
                    Ok(array_has(col(column.clone()), lit(*value)))
                }

                _ => Err(FilterError::InvalidBooleanFilterOperation(
                    self.clone(),
                    field.clone().into(),
                )),
            },
            Self::Nullable(value) => match field.data_type() {
                DataType::Boolean => {
                    if let Some(value) = value {
                        Ok(col(column.clone()).eq(lit(*value)))
                    } else {
                        Ok(col(column.clone()).is_null())
                    }
                }

                DataType::List(field) | DataType::ListView(field)
                    if field.data_type() == &DataType::Boolean =>
                {
                    // `ANY` semantics
                    if let Some(value) = value {
                        Ok(array_has(col(column.clone()), lit(*value)))
                    } else {
                        Ok(col(column.clone()).is_null().or(array_element(
                            array_sort(col(column.clone()), lit("ASC"), lit("NULLS FIRST")),
                            lit(1),
                        )
                        .is_null()))
                    }
                }

                _ => Err(FilterError::InvalidBooleanFilterOperation(
                    self.clone(),
                    field.clone().into(),
                )),
            },
        }
    }

    pub fn operand_text(&self) -> &str {
        match self {
            Self::NonNullable(value) => {
                if *value {
                    "true"
                } else {
                    "false"
                }
            }
            Self::Nullable(value) => {
                if let Some(value) = value {
                    if *value { "true" } else { "false" }
                } else {
                    "null"
                }
            }
        }
    }

    pub fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        column_name: &str,
        action: &mut super::FilterUiAction,
    ) {
        super::basic_operation_ui(ui, column_name, "is");

        let clicked = match self {
            Self::NonNullable(query) => {
                ui.re_radio_value(query, true, "true").clicked()
                    || ui.re_radio_value(query, false, "false").clicked()
            }

            Self::Nullable(query) => {
                ui.re_radio_value(query, Some(true), "true").clicked()
                    || ui.re_radio_value(query, Some(false), "false").clicked()
                    || ui.re_radio_value(query, None, "null").clicked()
            }
        };

        if clicked {
            *action = FilterUiAction::CommitStateToBlueprint;
        }
    }
}
