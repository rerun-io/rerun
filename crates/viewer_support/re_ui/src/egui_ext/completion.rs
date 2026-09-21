//! A code completion popup over a [`TextEdit`].
//!
//! Copied from <https://github.com/emilk/egui/pull/8529>, adapted to the public API of the egui
//! release we are on: the caller hands over the text buffer and a closure that builds the
//! [`TextEdit`], and mirrors the text edit's [`EventFilter`] on the popup.

use core::ops::Range;

use egui::text::{CCursor, CCursorRange, CharIndex};
use egui::text_edit::{TextEditOutput, TextEditState};
use egui::{
    Atom, AtomExt as _, Atoms, Button, Event, EventFilter, Id, InputState, IntoAtoms, Key, Popup,
    PopupKind, RectAlign, ScrollArea, TextBuffer, TextEdit, Ui, WidgetText, vec2,
};

/// One item in a [`CompletionPopup`].
#[derive(Clone)]
pub struct Suggestion {
    /// The text that replaces the current word when this suggestion is accepted.
    pub insert: String,

    /// What to show in the popup. Defaults to [`Self::insert`].
    pub content: Atoms<'static>,
}

impl Suggestion {
    /// A suggestion that inserts `insert`, showing it in the popup as-is.
    pub fn new(insert: impl Into<String>) -> Self {
        let insert = insert.into();
        Self {
            content: insert.clone().into_atoms(),
            insert,
        }
    }

    /// What to show in the popup instead of [`Self::insert`].
    #[inline]
    pub fn content(mut self, content: impl IntoAtoms<'static>) -> Self {
        self.content = content.into_atoms();
        self
    }

    /// Add weak text to the far right of the suggestion, truncated when the popup is narrow.
    #[inline]
    pub fn description(mut self, description: impl Into<WidgetText>) -> Self {
        self.content.push_right(Atom::grow());
        self.content
            .push_right(description.into().weak().atom_shrink(true));
        self
    }
}

/// What the [`CompletionPopup`] asks for suggestions for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionQuery<'a> {
    /// The full text of the [`TextEdit`].
    pub text: &'a str,

    /// The word that ends at the cursor (see [`CompletionPopup::word_boundary`]).
    pub word: &'a str,

    /// Where in [`Self::text`] the word is, as char indices.
    pub word_range: Range<CharIndex>,
}

impl CompletionQuery<'_> {
    /// Is the word at the very start of the text?
    pub fn is_at_start(&self) -> bool {
        self.word_range.start == CharIndex(0)
    }
}

/// The result of [`CompletionPopup::show`].
pub struct CompletionOutput {
    /// The output of the wrapped [`TextEdit`].
    pub text_edit: TextEditOutput,

    /// The suggestion accepted this frame (with Enter, Tab, or a click), if any.
    pub accepted: Option<Suggestion>,

    /// Is the popup visible?
    pub is_open: bool,
}

/// State stored between frames.
#[derive(Clone, Default)]
struct CompletionState {
    /// Index of the selected suggestion.
    selected: usize,

    /// The word under the cursor when the selection was last used.
    ///
    /// When the user types, the word and thus the suggestions change,
    /// and the old selection index would point at an unrelated suggestion.
    /// So when the word differs from this, the selection resets to the first suggestion.
    selected_word: String,

    /// Is the popup open? Updated at the end of each frame.
    open: bool,

    /// The word for which the user dismissed the popup with Escape.
    /// The popup stays hidden until the word changes.
    dismissed_word: Option<String>,
}

impl CompletionState {
    fn load(ui: &Ui, id: Id) -> Self {
        ui.data(|data| data.get_temp(id)).unwrap_or_default()
    }

    fn store(self, ui: &Ui, id: Id) {
        ui.data_mut(|data| data.insert_temp(id, self));
    }
}

/// Keys pressed while the popup was open, consumed before the [`TextEdit`] sees them.
#[derive(Default)]
struct PopupKeys {
    /// How many times `ArrowDown` was pressed.
    down: usize,

    /// How many times `ArrowUp` was pressed.
    up: usize,

    accept: bool,
    dismiss: bool,
}

impl PopupKeys {
    fn moved_selection(&self) -> bool {
        self.down != self.up
    }
}

/// A code completion popup over a [`TextEdit`].
///
/// Shows a list of suggestions for the word under the cursor.
/// Use the arrow keys to select one, and Enter or Tab to accept it.
/// Escape closes the popup until the word changes.
/// Clicking a suggestion also accepts it.
///
/// Keyboard focus stays in the [`TextEdit`] the whole time.
///
/// The popup starts intercepting keys the frame after it opens,
/// so an accept key pressed in the same frame as the popup appears goes to the [`TextEdit`].
///
/// ```
/// # egui::__run_test_ui(|ui| {
/// # let mut text = String::new();
/// let commands = ["/help", "/clear", "/quit"];
/// let output = re_ui::egui_ext::CompletionPopup::new(ui.make_persistent_id("prompt")).show(
///     ui,
///     &mut text,
///     |text| egui::TextEdit::singleline(text).hint_text("Type / for commands"),
///     |query| {
///         if !query.is_at_start() {
///             return vec![];
///         }
///         commands
///             .iter()
///             .filter(|command| command.starts_with(query.word))
///             .map(|command| re_ui::egui_ext::Suggestion::new(format!("{command} ")))
///             .collect()
///     },
/// );
/// if let Some(accepted) = output.accepted {
///     println!("Accepted {}", accepted.insert);
/// }
/// # });
/// ```
#[must_use = "You should call .show()"]
pub struct CompletionPopup<'a> {
    id: Id,
    is_word_boundary: Box<dyn Fn(char) -> bool + 'a>,
    align: RectAlign,
    max_height: f32,
    accept_keys: Vec<Key>,
    event_filter: EventFilter,
}

impl<'a> CompletionPopup<'a> {
    /// The `id` is given to the [`TextEdit`], and is also used to store the popup state.
    ///
    /// It must be unique, so if you show several completion popups (e.g. one per tab),
    /// use e.g. [`Ui::make_persistent_id`] rather than a constant [`Id`].
    pub fn new(id: Id) -> Self {
        Self {
            id,
            is_word_boundary: Box::new(char::is_whitespace),
            align: RectAlign::TOP_START,
            max_height: 200.0,
            accept_keys: vec![Key::Enter, Key::Tab],
            event_filter: EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            },
        }
    }

    /// Which characters separate words? The current word is the text
    /// between the last such character and the cursor.
    ///
    /// Default: [`char::is_whitespace`].
    #[inline]
    pub fn word_boundary(mut self, is_word_boundary: impl Fn(char) -> bool + 'a) -> Self {
        self.is_word_boundary = Box::new(is_word_boundary);
        self
    }

    /// Where to place the popup relative to the [`TextEdit`].
    /// If there is no room, the vertically flipped alignment is used instead.
    ///
    /// Default: [`RectAlign::TOP_START`].
    #[inline]
    pub fn align(mut self, align: RectAlign) -> Self {
        self.align = align;
        self
    }

    /// Maximum height of the popup before it starts to scroll.
    ///
    /// Default: 200.0.
    #[inline]
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height;
        self
    }

    /// Which keys accept the selected suggestion?
    ///
    /// Default: Enter and Tab.
    #[inline]
    pub fn accept_keys(mut self, accept_keys: impl Into<Vec<Key>>) -> Self {
        self.accept_keys = accept_keys.into();
        self
    }

    /// The [`TextEdit::event_filter`] to use. Set it here instead of on the [`TextEdit`],
    /// since the popup needs to add Tab and Escape to it while it is open.
    ///
    /// Default: the [`TextEdit`] default (arrow keys only).
    #[inline]
    pub fn event_filter(mut self, event_filter: EventFilter) -> Self {
        self.event_filter = event_filter;
        self
    }

    /// Show the [`TextEdit`] built by `text_edit` over `text` and, when `suggest` returns
    /// anything for the word under the cursor, the popup.
    ///
    /// The [`TextEdit`] is given the [`Id`] of this popup and the [`Self::event_filter`].
    ///
    /// `suggest` is called once per frame, after the [`TextEdit`] has handled input.
    pub fn show(
        self,
        ui: &mut Ui,
        text: &mut dyn TextBuffer,
        text_edit: impl for<'t> FnOnce(&'t mut dyn TextBuffer) -> TextEdit<'t>,
        suggest: impl FnOnce(&CompletionQuery<'_>) -> Vec<Suggestion>,
    ) -> CompletionOutput {
        let Self {
            id,
            is_word_boundary,
            align,
            max_height,
            accept_keys,
            event_filter,
        } = self;

        let mut state = CompletionState::load(ui, id);

        // Consume the popup keys before the `TextEdit` sees them.
        // We act on them after the `TextEdit` has run, when we know the current text and cursor.
        let mut keys = PopupKeys::default();
        if state.open {
            ui.input_mut(|input| {
                keys.down = consume_unmodified_key(input, Key::ArrowDown);
                keys.up = consume_unmodified_key(input, Key::ArrowUp);
                for &key in &accept_keys {
                    if 0 < consume_unmodified_key(input, key) {
                        keys.accept = true;
                    }
                }
                keys.dismiss = 0 < consume_unmodified_key(input, Key::Escape);
            });
        }

        let output = text_edit(&mut *text)
            .id(id)
            .event_filter(EventFilter {
                // Keep focus on Tab and Escape while the popup is open, so they can act on the popup:
                tab: event_filter.tab || state.open,
                escape: event_filter.escape || state.open,
                ..event_filter
            })
            .show(ui);
        let response = output.response.response.clone();

        let cursor = output.cursor_range.map_or_else(
            || text.as_str().chars().count(),
            |range| range.primary.index.0,
        );
        let word_range = word_range_before_cursor(text.as_str(), cursor, &*is_word_boundary);
        let word = text.char_range(word_range.clone()).to_owned();

        if keys.dismiss {
            state.dismissed_word = Some(word.clone());
        }

        let mut suggestions = if state.dismissed_word.as_deref() == Some(word.as_str()) {
            vec![]
        } else {
            suggest(&CompletionQuery {
                text: text.as_str(),
                word: &word,
                word_range: word_range.clone(),
            })
        };

        if state.selected_word != word {
            state.selected = 0;
            state.selected_word = word.clone();
        }
        if !suggestions.is_empty() {
            let len = suggestions.len();
            state.selected = (state.selected + keys.down + (len - keys.up % len)) % len;
        }

        let mut accepted_index = (keys.accept && !suggestions.is_empty()).then_some(state.selected);

        // Don't show the popup in the frame we accept, to avoid a one-frame flash of stale suggestions:
        let is_open = response.has_focus() && !suggestions.is_empty() && accepted_index.is_none();

        // The popup is as wide as the text edit, whatever the suggestions' natural width:
        // an `Area` only treats its width as a default and grows to fit long descriptions.
        let width = response.rect.width();
        let clicked = Popup::from_response(&response)
            .id(id.with("completion_popup"))
            .kind(PopupKind::Popup)
            .open(is_open)
            .align(align)
            .align_alternatives(&[align.flipped_y()])
            .width(width)
            .show(|ui| {
                ui.set_max_width(ui.available_width().min(width));
                ScrollArea::vertical()
                    .max_height(max_height)
                    .show(ui, |ui| {
                        let pointer_moved = ui.input(|input| input.pointer.is_moving());
                        let mut clicked = None;
                        for (i, suggestion) in suggestions.iter().enumerate() {
                            let is_selected = i == state.selected;
                            let response = ui.add(
                                Button::selectable(is_selected, suggestion.content.clone())
                                    .truncate()
                                    .min_size(vec2(ui.available_width(), 0.0)),
                            );
                            if is_selected && keys.moved_selection() {
                                response.scroll_to_me(None);
                            }
                            if response.hovered() && pointer_moved {
                                state.selected = i;
                            }
                            if response.clicked() {
                                clicked = Some(i);
                            }
                        }
                        clicked
                    })
                    .inner
            })
            .and_then(|inner| inner.inner);

        if clicked.is_some() {
            accepted_index = clicked;
            // Clicking the popup took focus from the text field, so give it back:
            ui.memory_mut(|mem| mem.request_focus(id));
        }

        let accepted = accepted_index
            .filter(|&i| i < suggestions.len())
            .map(|i| suggestions.swap_remove(i));
        if let Some(accepted) = &accepted {
            text.delete_char_range(word_range.clone());
            let mut ccursor = CCursor::new(word_range.start);
            text.insert_text_at(&mut ccursor, &accepted.insert, usize::MAX);

            let mut text_edit_state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
            text_edit_state
                .cursor
                .set_char_range(Some(CCursorRange::one(ccursor)));
            text_edit_state.store(ui.ctx(), id);

            state.dismissed_word = None;
            state.selected = 0;
        }

        state.open = is_open;
        state.store(ui, id);

        CompletionOutput {
            text_edit: output,
            accepted,
            is_open,
        }
    }
}

/// Consume presses of `key` with no modifiers held, returning how many there were.
///
/// Unlike [`InputState::consume_key`], this leaves e.g. Shift+Enter and Shift+ArrowDown alone,
/// so they still reach the [`TextEdit`].
fn consume_unmodified_key(input: &mut InputState, key: Key) -> usize {
    let mut count = 0;
    input.events.retain(|event| {
        let is_match = matches!(
            event,
            Event::Key {
                key: event_key,
                modifiers,
                pressed: true,
                ..
            } if *event_key == key && modifiers.is_none()
        );
        count += usize::from(is_match);
        !is_match
    });
    count
}

/// The char range of the word that ends at the cursor (a char index).
fn word_range_before_cursor(
    text: &str,
    cursor: usize,
    is_word_boundary: &dyn Fn(char) -> bool,
) -> Range<CharIndex> {
    let start = text
        .chars()
        .take(cursor)
        .enumerate()
        .filter(|&(_, c)| is_word_boundary(c))
        .last()
        .map_or(0, |(i, _)| i + 1);
    CharIndex(start)..CharIndex(cursor)
}

#[cfg(test)]
mod tests {
    use egui::accesskit::Role;
    use egui::{KeyboardShortcut, Modifiers};
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    const COMMANDS: &[&str] = &[
        "/clear",
        "/compact",
        "/config",
        "/grill-me",
        "/grill-with-docs",
        "/help",
    ];

    fn id() -> Id {
        Id::unique("prompt")
    }

    /// Commands are only valid at the start of the prompt.
    fn suggest_commands(query: &CompletionQuery<'_>) -> Vec<Suggestion> {
        if !query.is_at_start() || !query.word.starts_with('/') {
            return vec![];
        }
        COMMANDS
            .iter()
            .filter(|command| command.starts_with(query.word))
            .map(|command| Suggestion::new(format!("{command} ")))
            .collect()
    }

    #[derive(Default)]
    struct State {
        text: String,
        submitted: Option<String>,
        popup_open: bool,
    }

    fn harness<'a>() -> Harness<'a, State> {
        harness_with(|text| TextEdit::singleline(text))
    }

    /// A chat composer: multiline, Shift+Enter for newline, Enter to send.
    fn chat_harness<'a>() -> Harness<'a, State> {
        harness_with(|text| {
            TextEdit::multiline(text)
                .return_key(KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter))
        })
    }

    fn harness_with<'a>(
        make_text_edit: impl for<'t> Fn(&'t mut dyn TextBuffer) -> TextEdit<'t> + 'a,
    ) -> Harness<'a, State> {
        let mut harness = Harness::new_ui_state(
            move |ui, state: &mut State| {
                // Without any font, `TextEdit` lays out an empty galley and clamps the cursor to 0.
                crate::apply_style_and_install_loaders(ui.ctx());

                let output = CompletionPopup::new(id()).show(
                    ui,
                    &mut state.text,
                    &make_text_edit,
                    suggest_commands,
                );
                state.popup_open = output.is_open;
                let response = &output.text_edit.response.response;
                let enter = response.has_focus()
                    && ui.input_mut(|input| {
                        !input.modifiers.shift && input.consume_key(Modifiers::NONE, Key::Enter)
                    });
                if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) || enter
                {
                    response.request_focus();
                    state.submitted = Some(core::mem::take(&mut state.text));
                }
            },
            State::default(),
        );
        harness.run();
        text_input(&harness).focus();
        harness.run();
        harness
    }

    fn text_input<'h>(harness: &'h Harness<'_, State>) -> egui_kittest::Node<'h> {
        harness
            .query_by_role(Role::TextInput)
            .or_else(|| harness.query_by_role(Role::MultilineTextInput))
            .expect("no text input")
    }

    fn text_input_has_focus(harness: &Harness<'_, State>) -> bool {
        text_input(harness).is_focused()
    }

    fn cursor(harness: &Harness<'_, State>) -> Option<usize> {
        TextEditState::load(&harness.ctx, id())
            .and_then(|state| state.cursor.char_range())
            .map(|range| range.primary.index.0)
    }

    #[test]
    fn word_range_before_cursor_works() {
        let range = |text, cursor| {
            let range = word_range_before_cursor(text, cursor, &char::is_whitespace);
            range.start.0..range.end.0
        };
        assert_eq!(range("", 0), 0..0);
        assert_eq!(range("/gr", 3), 0..3);
        assert_eq!(range("hello /gr", 9), 6..9);
        assert_eq!(range("hello /gr", 6), 6..6);
        assert_eq!(range("hello /gr", 8), 6..8);
        assert_eq!(range("héllo wörld", 11), 6..11);

        let range = word_range_before_cursor("a.b", 3, &|c| c == '.');
        assert_eq!(range.start.0..range.end.0, 2..3);
    }

    #[test]
    fn arrows_and_enter_accept_completion() {
        let mut harness = harness();
        text_input(&harness).type_text("/gr");
        harness.run();
        assert!(harness.state().popup_open);

        harness.key_press(Key::ArrowDown);
        harness.run();
        assert!(text_input_has_focus(&harness));

        harness.key_press(Key::ArrowUp);
        harness.key_press(Key::ArrowUp);
        harness.run();

        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(
            harness.state().text,
            "/grill-with-docs ",
            "should wrap around"
        );
        assert_eq!(cursor(&harness), Some("/grill-with-docs ".len()));
        assert!(!harness.state().popup_open);
        assert!(text_input_has_focus(&harness));
        assert!(harness.state().submitted.is_none());
    }

    #[test]
    fn tab_accepts_completion_and_keeps_focus() {
        let mut harness = harness();
        text_input(&harness).type_text("/he");
        harness.run();
        harness.key_press(Key::Tab);
        harness.run();
        assert_eq!(harness.state().text, "/help ");
        assert!(text_input_has_focus(&harness));
    }

    #[test]
    fn escape_closes_popup_and_keeps_focus() {
        let mut harness = harness();
        text_input(&harness).type_text("/c");
        harness.run();
        assert!(harness.state().popup_open);

        harness.key_press(Key::Escape);
        harness.run();
        assert!(!harness.state().popup_open);
        assert!(text_input_has_focus(&harness));
        assert_eq!(harness.state().text, "/c");

        // Typing more reopens the popup:
        text_input(&harness).type_text("o");
        harness.run();
        assert!(harness.state().popup_open);
    }

    #[test]
    fn selection_resets_when_word_changes() {
        let mut harness = harness();
        text_input(&harness).type_text("/c");
        harness.run();
        harness.key_press(Key::ArrowDown); // selects /compact
        harness.run();

        text_input(&harness).type_text("o"); // /compact, /config
        harness.run();
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().text, "/compact ");
    }

    #[test]
    fn commands_only_complete_at_start_of_prompt() {
        let mut harness = harness();
        text_input(&harness).type_text("fix /he");
        harness.run();
        assert!(!harness.state().popup_open);
    }

    #[test]
    fn multiline_chat_composer() {
        let mut harness = chat_harness();
        text_input(&harness).type_text("/he");
        harness.run();
        assert!(harness.state().popup_open);

        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().text, "/help ", "Enter accepts, no newline");
        assert!(text_input_has_focus(&harness));
        assert!(harness.state().submitted.is_none());

        harness.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
        harness.run();
        assert_eq!(harness.state().text, "/help \n");

        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().submitted.as_deref(), Some("/help \n"));
        assert!(text_input_has_focus(&harness));
    }

    /// A long description must not widen the popup past the text edit, nor wrap onto more lines.
    #[test]
    fn long_descriptions_stay_within_the_text_edit() {
        let mut text = String::new();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1600.0, 900.0))
            .build_ui_state(
                |ui, text_edit_rect: &mut egui::Rect| {
                    crate::apply_style_and_install_loaders(ui.ctx());

                    egui::Panel::right("side")
                        .default_size(420.0)
                        .show(ui, |ui| {
                            let output = CompletionPopup::new(id()).show(
                                ui,
                                &mut text,
                                |text| TextEdit::multiline(text).desired_width(f32::INFINITY),
                                |query| {
                                    if !query.word.starts_with('/') {
                                        return vec![];
                                    }
                                    vec![
                                        Suggestion::new("/help ").description("h".repeat(300)),
                                        Suggestion::new("/clear ").description("Clear"),
                                    ]
                                },
                            );
                            *text_edit_rect = output.text_edit.response.response.rect;
                        });
                },
                egui::Rect::NOTHING,
            );
        harness.run();
        harness.get_by_role(Role::MultilineTextInput).focus();
        harness.run();
        harness.get_by_role(Role::MultilineTextInput).type_text("/");
        harness.run();
        harness.run();

        let text_edit_rect = *harness.state();
        let buttons: Vec<_> = harness.query_all_by_role(Role::Button).collect();
        assert_eq!(buttons.len(), 2);
        for button in buttons {
            let rect = button.rect();
            assert!(text_edit_rect.x_range().contains(rect.min.x), "{rect:?}");
            assert!(text_edit_rect.x_range().contains(rect.max.x), "{rect:?}");
            assert!(rect.height() < 30.0, "should be a single line: {rect:?}");
        }
    }

    #[test]
    fn enter_without_popup_submits() {
        let mut harness = harness();
        text_input(&harness).type_text("hello");
        harness.run();
        assert!(!harness.state().popup_open);

        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!(harness.state().submitted.as_deref(), Some("hello"));
        assert_eq!(harness.state().text, "");
        assert!(text_input_has_focus(&harness));
    }
}
