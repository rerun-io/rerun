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

    // Filter suggestions based on current text input.
    let filtered_suggestions: Vec<_> = suggestions
        .iter()
        .filter(|suggestion| {
            suggestion.starts_with(text_buffer.as_str()) && *suggestion != text_buffer.as_str()
        })
        .collect();

    let num_suggestions = filtered_suggestions.len();

    // In addition to mouse, allow also to select suggestions with up/down arrow keys and Enter.
    let (index_delta, enter_pressed) = ui.input(|i| {
        let delta =
            i.key_pressed(egui::Key::ArrowDown) as i32 - i.key_pressed(egui::Key::ArrowUp) as i32;
        (delta, i.key_pressed(egui::Key::Enter))
    });

    let suggestions_open =
        (response.has_focus() || response.lost_focus() || index_delta != 0) && num_suggestions > 0;

    // Persist the selected index using egui's temporary data storage if the suggestions popup is open.
    let selected_index: Option<usize> = if suggestions_open {
        let previous_index = ui.data(|d| d.get_temp::<usize>(response.id));
        let index = if index_delta != 0 {
            // (prev + n + delta) % n handles both directions correctly.
            let base = previous_index.map_or(if index_delta > 0 { usize::MAX } else { 0 }, |i| i);
            Some(
                (base
                    .wrapping_add(num_suggestions)
                    .wrapping_add_signed(index_delta as isize))
                    % num_suggestions,
            )
        } else {
            previous_index
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
        text_buffer.replace_with(suggestion);
        response.mark_changed();
        return response;
    }

    let width = response.rect.width();

    let mut changed = false;
    let suggestions_ui = |ui: &mut egui::Ui| {
        for (idx, suggestion) in filtered_suggestions.iter().enumerate() {
            let is_selected = selected_index == Some(idx);
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
