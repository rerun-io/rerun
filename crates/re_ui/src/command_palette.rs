use egui::{Align2, Key};

use crate::Command;

#[derive(Default)]
pub struct CommandPalette {
    visible: bool,
    text: String,
    selected_alternative: usize,
}

impl CommandPalette {
    pub fn toggle(&mut self) {
        self.visible ^= true;
    }

    /// Show the command palette, if it is visible.
    #[must_use = "Returns the command that was selected"]
    pub fn show(&mut self, egui_ctx: &egui::Context) -> Option<Command> {
        self.visible &= !egui_ctx
            .input_mut()
            .consume_key(Default::default(), Key::Escape);
        if !self.visible {
            self.text.clear();
            return None;
        }

        let width = 300.0;
        let max_height = 320.0;
        let y = egui_ctx.input().screen_rect().center().y - 0.5 * max_height;

        egui::Window::new("Command Palette")
            .title_bar(false)
            .anchor(Align2::CENTER_TOP, [0.0, y])
            .fixed_size([width, max_height])
            .show(egui_ctx, |ui| self.window_content(ui))?
            .inner?
    }

    #[must_use = "Returns the command that was selected"]
    fn window_content(&mut self, ui: &mut egui::Ui) -> Option<Command> {
        // Check _before_ we add the `TextEdit`, so it doesn't steal it.
        let enter_pressed = ui.input_mut().consume_key(Default::default(), Key::Enter);
        if enter_pressed {
            self.visible = false;
        }

        let text_response = ui.add(
            egui::TextEdit::singleline(&mut self.text)
                .desired_width(f32::INFINITY)
                .lock_focus(true),
        );
        text_response.request_focus();
        if text_response.changed() {
            self.selected_alternative = 0;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| self.alternatives(ui, enter_pressed))
            .inner
    }

    #[must_use = "Returns the command that was selected"]
    fn alternatives(&mut self, ui: &mut egui::Ui, enter_pressed: bool) -> Option<Command> {
        use strum::IntoEnumIterator as _;

        // TODO(emilk): nicer filtering
        let filter = self.text.to_lowercase();

        let item_height = 16.0;
        let font_id = egui::TextStyle::Button.resolve(ui.style());

        let mut num_alternatives: usize = 0;
        let mut selected_command = None;

        for (i, command) in Command::iter()
            .filter(|alt| alt.text().to_lowercase().contains(&filter))
            .enumerate()
        {
            let (text, tooltip) = command.text_and_tooltip();
            let kb_shortcut = command
                .kb_shortcut()
                .map(|shortcut| ui.ctx().format_shortcut(&shortcut))
                .unwrap_or_default();

            let (rect, response) = ui.allocate_at_least(
                egui::vec2(ui.available_width(), item_height),
                egui::Sense::click(),
            );

            let response = response.on_hover_text(tooltip);

            if response.clicked() {
                selected_command = Some(command);
                self.text.clear();
            }

            let selected = i == self.selected_alternative;
            let style = ui.style().interact_selectable(&response, selected);

            if selected {
                ui.painter()
                    .rect_filled(rect, style.rounding, ui.visuals().selection.bg_fill);

                if enter_pressed {
                    selected_command = Some(command);
                    self.text.clear();
                }

                ui.scroll_to_rect(rect, None);
            }

            // TODO(emilk): shorten long text using '…'
            ui.painter().text(
                rect.left_center(),
                Align2::LEFT_CENTER,
                text,
                font_id.clone(),
                style.text_color(),
            );
            ui.painter().text(
                rect.right_center(),
                Align2::RIGHT_CENTER,
                kb_shortcut,
                font_id.clone(),
                if selected {
                    style.text_color()
                } else {
                    ui.visuals().weak_text_color()
                },
            );

            num_alternatives += 1;
        }

        if num_alternatives == 0 {
            ui.weak("No matching results");
        }

        // Move up/down in the list:

        self.selected_alternative = self.selected_alternative.saturating_sub(
            ui.input_mut()
                .count_and_consume_key(Default::default(), Key::ArrowUp),
        );

        self.selected_alternative = self.selected_alternative.saturating_add(
            ui.input_mut()
                .count_and_consume_key(Default::default(), Key::ArrowDown),
        );

        self.selected_alternative = self
            .selected_alternative
            .clamp(0, num_alternatives.saturating_sub(1));

        selected_command
    }
}
