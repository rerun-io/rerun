use egui::Ui;
use re_log_types::EntityPath;

use re_ui::filter_widget::FilterMatcher;

/// A one-line entity path field, with the paths worth suggesting listed under it.
///
/// `on_change` is told each path as the user types, so the edited value can be previewed live.
///
/// Returns the settled path when editing ends, if it differs from `current`.
/// `current` must reflect any live previews applied by `on_change`.
/// Enter/Tab accepts the selected suggestion; Escape dismisses suggestions.
/// Editing ends on suggestion acceptance or when the text field loses focus (including Enter
/// without suggestions).
#[must_use]
pub fn entity_path_edit_ui(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    current: &EntityPath,
    suggestions: &dyn Fn() -> Vec<EntityPath>,
    on_change: &dyn Fn(&EntityPath),
) -> Option<EntityPath> {
    let state_id = ui.make_persistent_id(id_salt);
    let text_id = state_id.with("text");
    let mut text = ui
        .data_mut(|data| data.remove_temp::<String>(state_id))
        .unwrap_or_else(|| current.to_string());

    let completion = egui::CompletionPopup::new(text_id)
        .word_boundary(|_| false)
        .align(egui::emath::RectAlign::BOTTOM_START)
        .show(ui, egui::TextEdit::singleline(&mut text), |query| {
            let matcher = FilterMatcher::for_query(query.text);
            suggestions()
                .into_iter()
                .filter_map(|suggestion| {
                    let text = suggestion.to_string();
                    matcher.match_path(text.split('/'))?;
                    Some(egui::Suggestion::new(text))
                })
                .collect()
        });
    let mut output = completion.text_edit;

    if let Some(accepted) = completion.accepted {
        // CompletionPopup replaces only the prefix before the cursor.
        // Entity selection commits the entire path, including when the cursor is in the middle.
        output.response.surrender_focus();
        let chosen = EntityPath::from(accepted.insert);
        return (chosen != *current).then_some(chosen);
    }

    if output.response.gained_focus() {
        output
            .state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::select_all_str(&text)));
        output.state.store(ui.ctx(), text_id);
    }

    if output.response.changed() {
        on_change(&text.clone().into());
    }

    if output.response.lost_focus() {
        let chosen = EntityPath::from(text);
        return (chosen != *current).then_some(chosen);
    }

    if output.response.has_focus() {
        ui.data_mut(|data| data.insert_temp(state_id, text));
    }
    None
}
