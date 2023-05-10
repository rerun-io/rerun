use egui::Label;
use re_viewer_context::ViewerContext;

use super::SceneTextBox;

// --- Main view ---

#[derive(Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ViewTextBoxState {
    monospace: bool,
    word_wrap: bool,
}

impl Default for ViewTextBoxState {
    fn default() -> Self {
        Self {
            monospace: false,
            word_wrap: true,
        }
    }
}

impl ViewTextBoxState {
    pub fn selection_ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        crate::profile_function!();

        re_ui.selection_grid(ui, "text_config").show(ui, |ui| {
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

pub(crate) fn view_text_box(
    _ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTextBoxState,
    scene: &SceneTextBox,
) -> egui::Response {
    crate::profile_function!();

    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // TODO(jleibs): better handling for multiple results
                if scene.text_entries.len() == 1 {
                    let mut text = egui::RichText::new(&scene.text_entries[0].body);

                    if state.monospace {
                        text = text.monospace();
                    }

                    ui.add(Label::new(text).wrap(state.word_wrap));
                } else {
                    ui.label(format!(
                        "Unexpected number of text entries: {}. Limit your query to 1.",
                        scene.text_entries.len()
                    ));
                }
            })
    })
    .response
}
