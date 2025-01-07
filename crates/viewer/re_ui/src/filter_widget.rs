use eframe::epaint::Color32;
use egui::NumExt;

use crate::{list_item, UiExt as _};

/// State and UI for the filter widget.
///
/// The filter widget is designed as a toggle between a title widget and the filter text field.
/// [`Self`] is responsible for storing the widget state as well as the query text typed by the
/// user. [`FilterMatcher`] performs the actual filtering.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FilterState {
    /// The filter query string.
    ///
    /// If this is `None`, the filter is disabled (aka the text field is not visible).
    filter_query: Option<String>,

    /// Should the text field be focused?
    ///
    /// Set to `true` upon clicking on the search button.c
    #[serde(skip)]
    request_focus: bool,
}

impl FilterState {
    /// Return the filter if any.
    ///
    /// The widget must be enabled _and_ the filter string must not be empty.
    pub fn query(&self) -> Option<&str> {
        self.filter_query.as_deref().filter(|s| !s.is_empty())
    }

    pub fn filter(&self) -> FilterMatcher {
        FilterMatcher::new(self.query())
    }

    /// Display the filter widget.
    ///
    /// Note: this delegates to [`list_item::ListItem`], so you may need to setup the full span
    /// scope.
    pub fn ui(&mut self, ui: &mut egui::Ui, default_ui: impl FnOnce(&mut egui::Ui)) {
        let mut button_clicked = false;

        let icon = if self.filter_query.is_none() {
            &crate::icons::SEARCH
        } else {
            &crate::icons::CLOSE
        };

        list_item::list_item_scope(ui, ui.next_auto_id(), |ui| {
            ui.list_item()
                .interactive(false)
                .with_height(30.0)
                .show_flat(
                    ui,
                    list_item::CustomContent::new(|ui, _| {
                        if let Some(filter_query) = self.filter_query.as_mut() {
                            // we add additional spacing for aesthetic reasons (active text edits have a
                            // fat border)
                            ui.spacing_mut().text_edit_width =
                                (ui.max_rect().width() - 10.0).at_least(0.0);

                            let response = ui.text_edit_singleline(filter_query);

                            if self.request_focus {
                                self.request_focus = false;
                                response.request_focus();
                            }
                        } else {
                            default_ui(ui);
                        }
                    })
                    .action_button(icon, || {
                        button_clicked = true;
                    }),
                );
        });

        // defer button handling because we can't mutably borrow `self` in both closures above
        if button_clicked {
            if self.filter_query.is_none() {
                self.filter_query = Some(String::new());
                self.request_focus = true;
            } else {
                self.filter_query = None;
            }
        }
    }
}

// --

/// Full-text, case-insensitive matcher.
pub struct FilterMatcher {
    /// The lowercase version of the query string.
    ///
    /// If this is `None` or `Some("")`, the matcher will accept any input.
    lowercase_query: Option<String>,
}

impl FilterMatcher {
    pub fn new(query: Option<&str>) -> Self {
        Self {
            lowercase_query: query.map(|s| s.to_lowercase()),
        }
    }

    pub fn matches(&self, text: &str) -> bool {
        match self.lowercase_query.as_deref() {
            None | Some("") => true,
            Some(query) => text.to_lowercase().contains(query),
        }
    }

    pub fn matches_formatted(&self, ctx: &egui::Context, text: &str) -> Option<egui::WidgetText> {
        match self.lowercase_query.as_deref() {
            None | Some("") => Some(text.into()),

            Some(query) => {
                let lower_case_text = text.to_lowercase();

                if !lower_case_text.contains(query) {
                    return None;
                }

                let mut job = egui::text::LayoutJob::default();

                let mut start = 0;
                while let Some(index) = lower_case_text[start..].find(query) {
                    //highlight_builder.append()
                    job.append(
                        &text[start..start + index],
                        0.0,
                        egui::TextFormat {
                            font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                            color: Color32::PLACEHOLDER,
                            ..Default::default()
                        },
                    );

                    job.append(
                        &text[start + index..start + index + query.len()],
                        0.0,
                        egui::TextFormat {
                            font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                            color: Color32::PLACEHOLDER,
                            background: ctx.style().visuals.selection.bg_fill,
                            ..Default::default()
                        },
                    );

                    start += index + query.len();
                }

                job.append(
                    &text[start..],
                    0.0,
                    egui::TextFormat {
                        font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                        color: Color32::PLACEHOLDER,
                        ..Default::default()
                    },
                );

                Some(job.into())
            }
        }
    }
}
