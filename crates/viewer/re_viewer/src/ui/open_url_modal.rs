use std::str::FromStr as _;

use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{UICommand, UiExt as _};

use crate::open_url::ViewerOpenUrl;

#[derive(Default)]
pub struct OpenUrlModal {
    modal: ModalHandler,
    url: String,
    just_opened: bool,
}

impl OpenUrlModal {
    pub fn open(&mut self) {
        self.modal.open();
        self.just_opened = true;
    }

    pub fn ui(&mut self, ui: &egui::Ui) {
        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Open from URL").max_width(400.0),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Paste a URL below.")
                            .color(ui.visuals().strong_text_color()),
                    );

                    // Repeat shortcut on the right to remind users of how to open this modal quickly.
                    let shortcut_text = UICommand::OpenUrl
                        .formatted_kb_shortcut(ui.ctx())
                        .unwrap_or_default();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(shortcut_text).weak());
                    });
                });

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

                let open_url = ViewerOpenUrl::from_str(&self.url);
                let can_import = match &open_url {
                    Ok(url) => {
                        let description = url.open_description();
                        if let Some(target_short) = description.target_short {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", description.category));
                                ui.strong(target_short);
                            });
                        } else {
                            ui.label(description.category);
                        }

                        true
                    }
                    // Our parse errors aren't terribly informative when you're just typing malformed links.
                    Err(_err) => {
                        ui.error_label(
                            "Can't open this link - it doesn't appear to be a valid URL.",
                        );
                        false
                    }
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button_width = 50.0; // TODO(andreas): can we standardize this in the modals?

                    let open_response = ui.add_enabled(
                        can_import,
                        egui::Button::new("Open").min_size(egui::vec2(button_width, 0.0)),
                    );
                    if open_response.clicked()
                        || can_import && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        ui.ctx().open_url(egui::OpenUrl::same_tab(self.url.clone()));
                        ui.close();
                    }

                    let cancel_response =
                        ui.add(egui::Button::new("Cancel").min_size(egui::vec2(button_width, 0.0)));
                    if cancel_response.clicked() {
                        ui.close();
                    }
                });
            },
        );

        self.just_opened = false;
    }
}
