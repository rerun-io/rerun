use egui::Label;
use re_viewer_context::ViewerContext;

use super::SceneTextbox;

// --- Main view ---

#[derive(Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewTextboxState {
    monospace: bool,
    word_wrap: bool,
}

impl ViewTextboxState {
    pub fn selection_ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        crate::profile_function!();

        re_ui.selection_grid(ui, "log_config").show(ui, |ui| {
            re_ui.grid_left_hand_label(ui, "Text style");
            ui.vertical(|ui| {
                ui.radio_value(&mut self.monospace, false, "Proportional");
                ui.radio_value(&mut self.monospace, true, "Monospace");
                ui.checkbox(&mut self.word_wrap, "Word Wrap");
            });
            ui.end_row();
        });
    }
}

pub(crate) fn view_textbox(
    _ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTextboxState,
    scene: &SceneTextbox,
) -> egui::Response {
    crate::profile_function!();

    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
            // TODO(jleibs): better handling for multiple results
            if scene.text_entries.len() == 1 {
                let mut text = egui::RichText::new(&scene.text_entries[0].body);

                if state.monospace {
                    text = text.monospace();
                }

                ui.add(Label::new(text).wrap(state.word_wrap));
            } else {
                ui.label(format!(
                    "Unepxected number of text entries: {}. Limit your query to 1.",
                    scene.text_entries.len()
                ));
            }
        })
    })
    .response
}
