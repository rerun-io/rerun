use crate::theme::class;
use crate::theme::class::ClassesExt as _;
use crate::{DesignTokens, Size, TextEditVisuals, UiExt as _, Variant};
use egui::class::{Classes, HasClasses as _};
use egui::{Align2, Atoms, IntoAtoms, Response, TextBuffer, TextEdit, Ui, Vec2, Widget};

/// Wrapper around eguis [`TextEdit`] that applies reruns styling
pub struct ReTextEdit<'a> {
    text_edit: TextEdit<'a>,
    size: Size,
    variant: TextEditVariant,
    prefix: Atoms<'static>,
    suffix: Atoms<'static>,
}

/// The visual variant of the text edit.
///
/// Applied to an [`egui::TextEdit`] as a class name that [`crate::ReTheme`] resolves back into
/// colors from the design tokens, the same way [`crate::Variant`] works for buttons.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TextEditVariant {
    /// No bg fill and an outline stroke. Use when the textedit is standalone and doesn't edit a
    /// value (e.g. search bar).
    #[default]
    Outlined,

    /// Filled to look similar to buttons and other widgets. Use in forms or when editing some
    /// value.
    Filled,
}

impl TextEditVariant {
    pub const CLASS_OUTLINED: &'static str = "rerun::text_edit::outlined";
    pub const CLASS_FILLED: &'static str = "rerun::text_edit::filled";

    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Outlined => Self::CLASS_OUTLINED,
            Self::Filled => Self::CLASS_FILLED,
        }
    }

    fn from_class_name(name: &str) -> Option<Self> {
        // An `egui::DragValue` forwards its classes to the text edit it turns into while being
        // typed into, so the button variants are accepted too and map onto their closest text edit
        // twin. That is what keeps an outlined drag value outlined while it is being edited.
        match name {
            Self::CLASS_OUTLINED | Variant::CLASS_OUTLINED => Some(Self::Outlined),
            Self::CLASS_FILLED
            | Variant::CLASS_PRIMARY
            | Variant::CLASS_SECONDARY
            | Variant::CLASS_GHOST
            | Variant::CLASS_OPENED => Some(Self::Filled),
            _ => None,
        }
    }

    /// The variant a text edit asks for, if any. The last variant class wins.
    pub fn from_classes(classes: &Classes) -> Option<Self> {
        classes.last_class(Self::from_class_name)
    }

    pub fn visuals(self, tokens: &DesignTokens) -> &TextEditVisuals {
        match self {
            Self::Outlined => &tokens.text_edit_outlined,
            Self::Filled => &tokens.text_edit_filled,
        }
    }
}

impl ReTextEdit<'_> {
    pub fn singleline(text: &mut dyn TextBuffer) -> ReTextEdit<'_> {
        ReTextEdit {
            text_edit: TextEdit::singleline(text).align(Align2::LEFT_CENTER),
            size: Size::Small,
            variant: TextEditVariant::Outlined,
            prefix: Atoms::default(),
            suffix: Atoms::default(),
        }
    }

    pub fn multiline(text: &mut dyn TextBuffer) -> ReTextEdit<'_> {
        ReTextEdit {
            text_edit: TextEdit::multiline(text),
            size: Size::Small,
            variant: TextEditVariant::Outlined,
            prefix: Atoms::default(),
            suffix: Atoms::default(),
        }
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Pin the id of the underlying [`TextEdit`].
    ///
    /// Needed to query or request focus for the text edit before it is shown.
    pub fn id(mut self, id: egui::Id) -> Self {
        self.text_edit = self.text_edit.id(id);
        self
    }

    /// How wide the field is, instead of egui's fixed default width.
    pub fn desired_width(mut self, desired_width: f32) -> Self {
        self.text_edit = self.text_edit.desired_width(desired_width);
        self
    }

    /// Show each character as a dot, e.g. for a password or an API key.
    pub fn password(mut self, password: bool) -> Self {
        self.text_edit = self.text_edit.password(password);
        self
    }

    pub fn variant(mut self, variant: TextEditVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn prefix(mut self, atoms: impl IntoAtoms<'static>) -> Self {
        self.prefix = atoms.into_atoms();
        self
    }

    pub fn suffix(mut self, atoms: impl IntoAtoms<'static>) -> Self {
        self.suffix = atoms.into_atoms();
        self
    }

    pub fn hint_text(mut self, text: impl IntoAtoms<'static>) -> Self {
        self.text_edit = self.text_edit.hint_text(text);
        self
    }

    /// Outline the field in the error color, to show that its content is invalid.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.text_edit = self.text_edit.invalid(invalid);
        self
    }

    pub fn atom_ui(self, ui: &mut Ui) -> egui::WidgetAtomResponse {
        let Self {
            mut text_edit,
            size,
            variant,
            mut prefix,
            mut suffix,
        } = self;
        let tokens = ui.tokens();

        text_edit = text_edit
            .with_class(variant.class_name())
            .with_class(size.class_name());

        if !prefix.is_empty() {
            prefix.map_images(|i| i.tint(tokens.text_strong));
            text_edit = text_edit.prefix(prefix);
        }
        if !suffix.is_empty() {
            suffix.map_images(|i| i.tint(tokens.text_strong));
            text_edit = text_edit.suffix(suffix);
        }

        // The class already carries the size to the theme, which is what sizes the field.
        // `Custom` is the exception: it borrows the class of the nearest standard size, so its
        // exact height has to travel on the widget.
        if let Size::Custom { .. } = size {
            text_edit = text_edit.min_size(Vec2::new(0.0, size.height()));
        }

        text_edit.show(ui).response
    }
}

impl Widget for ReTextEdit<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.atom_ui(ui).response
    }
}

/// Rerun's styling, as extensions to [`egui::TextEdit`].
pub trait TextEditExt: Sized {
    /// Outline the field in the error color, to show that its content is invalid.
    ///
    /// Validating the content is up to the caller.
    fn invalid(self, invalid: bool) -> Self;
}

impl TextEditExt for TextEdit<'_> {
    fn invalid(self, invalid: bool) -> Self {
        self.with_class_if(class::INVALID, invalid)
    }
}

/// Text edit with autocomplete suggestions popup.
///
/// Shows an editable `text_buffer` with matching entries from `suggestions`
/// as selectable options in a popup below the text edit.
///
/// `hint_text` is an optional placeholder text shown when the buffer is empty.
///
/// `invalid_hint_text` can be used to highlight invalid input and show an info
/// label with the text on top of the suggestion list. Input validation itself
/// has to be done outside of this function.
///
/// Leading whitespace in a suggestion is treated as display-only indentation:
/// it's kept as indentation in the popup, but stripped for filtering and when
/// the suggestion is written back into the buffer.
pub fn autocomplete_text_edit(
    ui: &mut egui::Ui,
    text_buffer: &mut dyn egui::TextBuffer,
    suggestions: &[String],
    empty_hint_text: Option<impl Into<egui::WidgetText>>,
    invalid_hint_text: Option<impl Into<String>>,
) -> egui::Response {
    // Grow with the available width instead of egui's default fixed `text_edit_width` cap.
    let mut text_edit = egui::TextEdit::singleline(text_buffer).desired_width(ui.available_width());
    if let Some(hint) = empty_hint_text {
        text_edit = text_edit.hint_text(hint);
    }

    let invalid = invalid_hint_text.is_some();
    if invalid {
        text_edit = text_edit.text_color(ui.visuals().error_fg_color);
    }

    let mut response = ui.add(text_edit.invalid(invalid));

    // Filter suggestions based on current text input.
    let filtered_suggestions: Vec<_> = suggestions
        .iter()
        .filter(|suggestion| {
            // Trim leading whitespace for input matching,
            // but keep it for visually indenting the suggestions.
            let value = suggestion.trim_start();
            value.starts_with(text_buffer.as_str()) && value != text_buffer.as_str()
        })
        .collect();

    let num_suggestions = filtered_suggestions.len();

    // In addition to mouse, allow also to select suggestions with up/down arrow keys and Enter.
    let (index_delta, enter_pressed) = ui.input(|i| {
        let delta =
            i.key_pressed(egui::Key::ArrowDown) as i32 - i.key_pressed(egui::Key::ArrowUp) as i32;
        (delta, i.key_pressed(egui::Key::Enter))
    });

    let suggestions_open = (response.has_focus() || response.lost_focus() || index_delta != 0)
        && (num_suggestions > 0 || invalid_hint_text.is_some());

    // Persist the selected index using egui's temporary data storage if the suggestions popup is open.
    let selected_index: Option<usize> = if suggestions_open {
        let previous_index = ui.data(|d| d.get_temp::<usize>(response.id));
        let index = if index_delta == 0 {
            previous_index
        } else {
            // (prev + n + delta) % n handles both directions correctly.
            let base = previous_index.unwrap_or(if index_delta > 0 { usize::MAX } else { 0 });
            Some(
                (base
                    .wrapping_add(num_suggestions)
                    .wrapping_add_signed(index_delta as isize))
                    % num_suggestions,
            )
        };
        if let Some(i) = index {
            ui.data_mut(|d| d.insert_temp(response.id, i));
        }
        index
    } else {
        ui.data_mut(|d| d.remove::<usize>(response.id));
        None
    };

    // If enter was pressed, confirm the selection and don't show the suggestion popup.
    if enter_pressed
        && let Some(idx) = selected_index
        && let Some(suggestion) = filtered_suggestions.get(idx)
    {
        text_buffer.replace_with(suggestion.trim_start());
        response.mark_changed();
        return response;
    }

    let width = response.rect.width();

    let mut changed = false;
    let suggestions_ui = |ui: &mut egui::Ui| {
        for (idx, suggestion) in filtered_suggestions.iter().enumerate() {
            let is_selected = selected_index == Some(idx);

            // Keep any leading indentation, then highlight the matched prefix against the completion.
            let value = suggestion.trim_start();
            let indent = &suggestion[..suggestion.len() - value.len()];
            let completion = value.strip_prefix(text_buffer.as_str()).unwrap_or("");

            let body = ui.style().text_styles[&egui::TextStyle::Body].clone();
            let mut layout_job = egui::text::LayoutJob::default();

            // Already typed part of the suggestion: "highlighted" as normal text.
            layout_job.append(
                &format!("{indent}{}", text_buffer.as_str()),
                0.0,
                egui::TextFormat::simple(body.clone(), ui.tokens().text_default),
            );
            // Completion remainder of the suggestion: subdued.
            layout_job.append(
                completion,
                0.0,
                egui::TextFormat::simple(body, ui.tokens().text_subdued),
            );

            let button = egui::Button::new(layout_job)
                .min_size(egui::vec2(width, 0.0))
                .selected(is_selected);
            let button_response = ui.add(button);

            if is_selected {
                // Make sure the selected item is visible also when using up/down keys.
                button_response.scroll_to_me(Some(egui::Align::Center));
            }

            if button_response.clicked() {
                changed = true;
                text_buffer.replace_with(value);
            }
        }
    };

    egui::Popup::from_response(&response)
        .open(suggestions_open)
        .show(|ui: &mut egui::Ui| {
            ui.set_width(width);

            // Show hint for invalid input always on top of the suggestions.
            if let Some(invalid_hint_text) = invalid_hint_text.map(Into::into) {
                ui.info_label(invalid_hint_text);
                if num_suggestions > 0 {
                    ui.add_space(ui.spacing().item_spacing.y);
                }
            }

            egui::ScrollArea::vertical()
                .min_scrolled_height(350.0)
                .max_height(350.0)
                .show(ui, suggestions_ui);
        });

    if changed {
        response.mark_changed();
    }

    response
}
