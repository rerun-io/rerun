use core::f32;
use std::str::FromStr as _;

use re_ui::UiExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};

use crate::open_url::ViewerImportUrl;

#[derive(Default)]
pub struct ImportUrlModal {
    modal: ModalHandler,
    url: String,
    just_opened: bool,
}

impl ImportUrlModal {
    pub fn open(&mut self) {
        self.modal.open();
        self.just_opened = true;
    }

    pub fn ui(&mut self, ui: &egui::Ui) {
        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Import URL"),
            |ui| {
                ui.label("Enter URL to import.");

                let edit_output = egui::TextEdit::singleline(&mut self.url)
                    .desired_width(f32::INFINITY)
                    .show(ui);

                // If we just opened the dialog, focus the text edit so user can just paste.
                if self.just_opened {
                    edit_output.response.request_focus();

                    // Pasting the clipboard is a cool idea until you realize that we may just have pasted a password.
                    // We can't read the clipboard contents on the web and we don't have a nice API for that on native right now,
                    // so let's not.
                    // ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }

                let import_url = ViewerImportUrl::from_str(&self.url);
                let can_import = match &import_url {
                    Ok(url) => {
                        ui.info_label(url.open_description());
                        true
                    }
                    // Our parse errors aren't terribly informative when you're just typing malformed links.
                    Err(_err) => {
                        ui.warning_label("Not a valid URL that can be opened.");
                        false
                    }
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(can_import, egui::Button::new("Ok"))
                        .clicked()
                        || can_import && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        // We already made sure that this is a valid import URL, so we we'll not _actually_ open it into a new tab.
                        // Using `new_tab` for these means that we'll switch to it which is what we want.
                        ui.ctx().open_url(egui::OpenUrl::new_tab(self.url.clone()));
                        ui.close();
                    }

                    if ui.button("Cancel").clicked() {
                        ui.close();
                    }
                });
            },
        );

        self.just_opened = false;
    }
}
