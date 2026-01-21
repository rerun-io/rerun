use crate::UiExt as _;

/// Text edit with autocomplete suggestions popup.
///
/// Shows an editable `text_buffer` with matching entries from `suggestions`
/// as selectable options in a popup below the text edit.
///
/// `hint_text` is an optional placeholder text shown when the buffer is empty.
pub fn autocomplete_text_edit(
    ui: &mut egui::Ui,
    text_buffer: &mut dyn egui::TextBuffer,
    suggestions: &[String],
    hint_text: Option<impl Into<egui::WidgetText>>,
) -> egui::Response {
    let mut text_edit = egui::TextEdit::singleline(text_buffer);
    if let Some(hint) = hint_text {
        text_edit = text_edit.hint_text(hint);
    }
    let mut response = ui.add(text_edit);

    // Filter suggestions based on current input.
    let filtered_suggestions: Vec<_> = suggestions
        .iter()
        .filter(|suggestion| {
            suggestion.starts_with(text_buffer.as_str()) && *suggestion != text_buffer.as_str()
        })
        .collect();

    let suggestions_open =
        (response.has_focus() || response.lost_focus()) && !filtered_suggestions.is_empty();

    let width = response.rect.width();

    let mut changed = false;
    let suggestions_ui = |ui: &mut egui::Ui| {
        for suggestion in &filtered_suggestions {
            let completion = suggestion.strip_prefix(text_buffer.as_str()).unwrap_or("");

            let mut layout_job = egui::text::LayoutJob::default();
            layout_job.append(
                text_buffer.as_str(),
                0.0,
                egui::TextFormat::simple(
                    ui.style().text_styles[&egui::TextStyle::Body].clone(),
                    ui.tokens().text_default,
                ),
            );
            layout_job.append(
                completion,
                0.0,
                egui::TextFormat::simple(
                    ui.style().text_styles[&egui::TextStyle::Body].clone(),
                    ui.tokens().text_subdued,
                ),
            );

            if ui
                .add(egui::Button::new(layout_job).min_size(egui::vec2(width, 0.0)))
                .clicked()
            {
                changed = true;
                text_buffer.replace_with(suggestion);
            }
        }
    };

    egui::Popup::from_response(&response)
        .style(crate::menu::menu_style())
        .open(suggestions_open)
        .show(|ui: &mut egui::Ui| {
            ui.set_width(width);

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
