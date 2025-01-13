use egui::{Color32, NumExt as _, Widget as _};
use itertools::Either;
use rand::random;

use crate::{list_item, UiExt as _};

/// State for the filter widget when it is toggled on.
#[derive(Debug, Clone)]
struct InnerState {
    /// The filter query string.
    filter_query: String,

    /// This ID is recreated every time the filter is toggled and tracks the current filtering
    /// session.
    ///
    /// This can be useful for client code to store session-specific state (e.g., the state of tree
    /// collapsed-ness).
    session_id: egui::Id,
}

impl Default for InnerState {
    fn default() -> Self {
        Self {
            filter_query: String::new(),

            // create a new session id each time the filter is toggled
            session_id: egui::Id::new(random::<u64>()),
        }
    }
}

/// State and UI for the filter widget.
///
/// The filter widget is designed as a toggle between a title widget and the filter text field.
/// [`Self`] is responsible for storing the widget state as well as the query text typed by the
/// user. [`FilterMatcher`] performs the actual filtering.
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    /// The current state of the filter widget.
    ///
    /// This is `None` when the filter is not active.
    inner_state: Option<InnerState>,

    /// Should the text field be focused?
    ///
    /// Set to `true` upon clicking on the search button.
    request_focus: bool,
}

impl FilterState {
    /// Return the filter if any.
    ///
    /// The widget must be enabled _and_ the filter string must not be empty.
    pub fn query(&self) -> Option<&str> {
        self.inner_state
            .as_ref()
            .map(|state| state.filter_query.as_str())
    }

    /// Return the current session ID of the filter widget, if active.
    pub fn session_id(&self) -> Option<egui::Id> {
        self.inner_state.as_ref().map(|state| state.session_id)
    }

    /// Return a filter matcher for the current query.
    pub fn filter(&self) -> FilterMatcher {
        FilterMatcher::new(self.query())
    }

    /// Display the filter widget.
    ///
    /// Note: this uses [`egui::Ui::available_width`] to determine the location of the right-aligned
    /// search button, as usual for [`list_item::ListItem`]-based widgets.
    pub fn ui(&mut self, ui: &mut egui::Ui, section_title: impl Into<egui::WidgetText>) {
        let mut button_clicked = false;

        let icon = if self.inner_state.is_none() {
            &crate::icons::SEARCH
        } else {
            &crate::icons::CLOSE
        };

        // precompute the title layout such that we know the size we need for the list item content
        let section_title = section_title.into();
        let galley = section_title.into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::FontSelection::default(),
        );
        let text_width = galley.size().x;

        list_item::list_item_scope(ui, ui.next_auto_id(), |ui| {
            ui.list_item()
                .interactive(false)
                .with_height(30.0)
                .show_flat(
                    ui,
                    list_item::CustomContent::new(|ui, _| {
                        if let Some(inner_state) = self.inner_state.as_mut() {
                            // we add additional spacing for aesthetic reasons (active text edits have a
                            // fat border)
                            ui.spacing_mut().text_edit_width =
                                (ui.max_rect().width() - 10.0).at_least(0.0);

                            let response =
                                egui::TextEdit::singleline(&mut inner_state.filter_query)
                                    .lock_focus(true)
                                    .ui(ui);

                            if self.request_focus {
                                self.request_focus = false;
                                response.request_focus();
                            } else if response.lost_focus() {
                                self.inner_state = None;
                            }
                        } else {
                            ui.label(galley);
                        }
                    })
                    .with_content_width(text_width)
                    .action_button(icon, || {
                        button_clicked = true;
                    }),
                );
        });

        // defer button handling because we can't mutably borrow `self` in both closures above
        if button_clicked {
            if self.inner_state.is_none() {
                self.inner_state = Some(InnerState::default());
                self.request_focus = true;
            } else {
                self.inner_state = None;
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
    fn new(query: Option<&str>) -> Self {
        Self {
            lowercase_query: query.map(|s| s.to_lowercase()),
        }
    }

    /// Is the filter set to match everything?
    ///
    /// Can be used by client code to short-circuit more expansive matching logic.
    pub fn matches_everything(&self) -> bool {
        self.lowercase_query.is_none()
    }

    /// Is the filter set to match nothing?
    ///
    /// Can be used by client code to short-circuit more expansive matching logic.
    pub fn matches_nothing(&self) -> bool {
        self.lowercase_query
            .as_ref()
            .is_some_and(|query| query.is_empty())
    }

    /// Does the given text match the filter?
    pub fn matches(&self, text: &str) -> bool {
        match self.lowercase_query.as_deref() {
            None => true,
            Some("") => false,
            Some(query) => text.to_lowercase().contains(query),
        }
    }

    /// Find all matches in the given text.
    ///
    /// Can be used to format the text, e.g. with [`format_matching_text`].
    pub fn find_matches(&self, text: &str) -> Option<impl Iterator<Item = (usize, usize)> + '_> {
        let query = match self.lowercase_query.as_deref() {
            None => {
                return Some(Either::Left(std::iter::empty()));
            }
            Some("") => {
                return None;
            }
            Some(query) => query,
        };

        let mut start = 0;
        let lower_case_text = text.to_lowercase();
        let query_len = query.len();

        if !lower_case_text.contains(query) {
            return None;
        }

        Some(Either::Right(std::iter::from_fn(move || {
            let index = lower_case_text[start..].find(query)?;
            let start_index = start + index;
            start = start_index + query_len;
            Some((start_index, start_index + query_len))
        })))
    }

    pub fn matches_formatted(&self, ctx: &egui::Context, text: &str) -> Option<egui::WidgetText> {
        self.find_matches(text)
            .map(|match_iter| format_matching_text(ctx, text, match_iter))
    }
}

/// Given a list of highlight sections defined by start/end indices and a string, produce a properly
/// highlighted [`egui::WidgetText`].
pub fn format_matching_text(
    ctx: &egui::Context,
    text: &str,
    match_iter: impl Iterator<Item = (usize, usize)>,
) -> egui::WidgetText {
    let mut current = 0;
    let mut job = egui::text::LayoutJob::default();

    for (start_idx, end_idx) in match_iter {
        if current < start_idx {
            job.append(
                &text[current..start_idx],
                0.0,
                egui::TextFormat {
                    font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                    color: Color32::PLACEHOLDER,
                    ..Default::default()
                },
            );
        }

        job.append(
            &text[start_idx..end_idx],
            0.0,
            egui::TextFormat {
                font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                color: Color32::PLACEHOLDER,
                background: ctx.style().visuals.selection.bg_fill,
                ..Default::default()
            },
        );

        current = end_idx;
    }

    if current < text.len() {
        job.append(
            &text[current..],
            0.0,
            egui::TextFormat {
                font_id: egui::TextStyle::Body.resolve(&ctx.style()),
                color: Color32::PLACEHOLDER,
                ..Default::default()
            },
        );
    }

    job.into()
}
