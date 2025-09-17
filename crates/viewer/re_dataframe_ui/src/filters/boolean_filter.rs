use arrow::datatypes::{DataType, Field};
use datafusion::common::Column;
use datafusion::logical_expr::{Expr, col, lit};
use datafusion::prelude::{array_element, array_has, array_sort};
use re_ui::UiExt as _;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{FilterError, FilterUiAction, Nullability};

/// A filter for a boolean column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BooleanFilter {
    /// Filter for strictly non-nullable columns.
    ///
    /// This includes non-nullable primitive datatypes, as well as outer AND inner non-nullable nested
    /// datatypes. In that case, the UI should not display the `null` option.
    NonNullable(bool),

    /// Filter for nullable columns.
    ///
    /// In this case, the UI should display a `null` option. A value of `None` means nulls should be
    /// matched.
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

    pub fn operand_text(&self) -> String {
        match self {
            Self::NonNullable(value) => value.to_string(),

            Self::Nullable(value) => {
                if let Some(value) = value {
                    value.to_string()
                } else {
                    "null".to_owned()
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

        let primitive = |ui: &egui::Ui, s: &str| {
            SyntaxHighlightedBuilder::primitive(s).into_widget_text(ui.style())
        };

        let mut clicked = false;
        match self {
            Self::NonNullable(query) => {
                clicked |= ui
                    .re_radio_value(query, true, primitive(ui, "true"))
                    .clicked();
                clicked |= ui
                    .re_radio_value(query, false, primitive(ui, "false"))
                    .clicked();
            }

            Self::Nullable(query) => {
                clicked |= ui
                    .re_radio_value(query, Some(true), primitive(ui, "true"))
                    .clicked();
                clicked |= ui
                    .re_radio_value(query, Some(false), primitive(ui, "false"))
                    .clicked();
                clicked |= ui
                    .re_radio_value(query, None, primitive(ui, "null"))
                    .clicked();
            }
        }

        if clicked {
            *action = FilterUiAction::CommitStateToBlueprint;
        }
    }
}
