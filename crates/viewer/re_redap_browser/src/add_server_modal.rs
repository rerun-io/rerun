use crate::context::Context;
use crate::servers::Command;
use re_grpc_client::redap;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::UiExt;

#[derive(Default)]
pub struct AddServerModal {
    modal: ModalHandler,
    url: String,
}

impl AddServerModal {
    pub fn open(&mut self) {
        self.url = "rerun://".to_owned();
        self.modal.open();
    }

    //TODO(ab): make that UI a form with a scheme popup, a host text field, and a pre-filled port field
    //TODO(ab): handle ESC and return
    pub fn ui(&mut self, ctx: &Context<'_>, ui: &egui::Ui) {
        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Add Server"),
            |ui, keep_open| {
                ui.label("URL:");
                ui.add(egui::TextEdit::singleline(&mut self.url).lock_focus(false));

                let origin = redap::Origin::try_from(self.url.as_ref());

                match &origin {
                    Ok(_) => {
                        ui.success_label("URL is valid");
                    }
                    Err(err) => {
                        ui.error_label(format!("Unable to parse server URL: {err}"));
                    }
                }

                ui.add_space(24.0);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Ok(origin) = origin {
                        if ui.button("Add").clicked() {
                            *keep_open = false;

                            let _ = ctx.command_sender.send(Command::AddServer(origin));
                        }
                    } else {
                        ui.add_enabled(false, egui::Button::new("Add"));
                    }

                    if ui.button("Cancel").clicked() {
                        *keep_open = false;
                    }
                });
            },
        );
    }
}
