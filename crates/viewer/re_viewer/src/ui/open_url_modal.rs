use std::str::FromStr as _;

use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{UICommand, UiExt as _};
use re_viewer_context::open_url::ViewerOpenUrl;

use crate::open_url_description::ViewerOpenUrlDescription;

#[derive(Default)]
pub struct OpenUrlModal {
    modal: ModalHandler,
    url: String,
    just_opened: bool,

    /// Used in tests to hide the platform dependent shortcut text.
    hide_shortcut: bool,
}

impl OpenUrlModal {
    pub fn open(&mut self) {
        self.modal.open();
        self.just_opened = true;
    }

    pub fn ui(&mut self, ui: &egui::Ui) {
        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Open from URL"),
            |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Paste a URL below.");

                    // Repeat shortcut on the right to remind users of how to open this modal quickly.
                    if !self.hide_shortcut {
                        let shortcut_text = UICommand::OpenUrl
                            .formatted_kb_shortcut(ui.ctx())
                            .unwrap_or_default();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(shortcut_text);
                        });
                    }
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
                    // ui.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }

                let open_url = ViewerOpenUrl::from_str(&self.url);
                let can_import = match &open_url {
                    Ok(url) => {
                        let description = ViewerOpenUrlDescription::from_url(url);
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
                        if self.url.is_empty() {
                            ui.error_label("Please paste a valid URL.");
                        } else {
                            ui.error_label(
                                "Can't open this link - it doesn't appear to be a valid URL.",
                            );
                        }
                        false
                    }
                };

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button_width = ui.tokens().modal_button_width;

                    let open_response = ui.add_enabled(
                        can_import,
                        egui::Button::new("Open").min_size(egui::vec2(button_width, 0.0)),
                    );
                    if open_response.clicked()
                        || can_import && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        ui.open_url(egui::OpenUrl::same_tab(self.url.clone()));
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

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;

    use crate::ui::OpenUrlModal;

    #[test]
    fn test_open_url_modal() {
        let mut modal = OpenUrlModal {
            hide_shortcut: true, // Shortcuts are platform specific, so they shouldn't show up in the screenshots.
            ..Default::default()
        };
        modal.open();

        let url = Mutex::new(String::new());

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::Vec2::new(450.0, 200.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                modal.url = url.lock().clone();
                modal.ui(ui);
            });

        *url.lock() = String::new();
        harness.run();
        harness.snapshot("open_url_modal__no_url");

        *url.lock() = "rerun://sandbox.redap.rerun.io:443/dataset/185998CF5EF38BF27f3e5ede1ca9a1a2?segment_id=ILIAD_5e938e3b_2023_07_28_10h_25m_47s".to_owned();
        harness.run();
        harness.snapshot("open_url_modal__valid_url");

        *url.lock() = "The shovel was a ground breaking invention.".to_owned();
        harness.run();
        harness.snapshot("open_url_modal__invalid_url");
    }
}
