use std::fmt::Formatter;

use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{FilterUiAction, action_from_text_edit_response};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
            Self::Eq => "==".fmt(f),
            Self::Ne => "!=".fmt(f),
            Self::Lt => "<".fmt(f),
            Self::Le => "<=".fmt(f),
            Self::Gt => ">".fmt(f),
            Self::Ge => ">=".fmt(f),
        }
    }
}

impl ComparisonOperator {
    pub const ALL: &'static [Self] = &[Self::Eq, Self::Ne, Self::Lt, Self::Le, Self::Gt, Self::Ge];

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
            Self::Eq => left == right,
            Self::Ne => left != right,
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
                for possible_op in crate::filters::ComparisonOperator::ALL {
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
