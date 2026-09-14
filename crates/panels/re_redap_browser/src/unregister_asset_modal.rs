//! Asking before an asset is unregistered.

use re_log_types::EntryId;
use re_log_types::external::re_types_core::SegmentId;
use re_quota_channel::send_crossbeam;
use re_ui::ReButton;
use re_ui::modal::{ModalHandler, ModalWrapper};

use crate::context::Context;
use crate::servers::Command;

/// Modal asking the user to confirm unregistering an asset.
///
/// Unregistering cannot be undone from the viewer, so it asks first.
#[derive(Default)]
pub struct UnregisterAssetModal {
    modal: ModalHandler,

    state: Option<State>,
}

/// The asset the modal is asking about while it is open.
struct State {
    origin: re_uri::Origin,
    dataset_id: EntryId,
    asset_id: SegmentId,
}

impl UnregisterAssetModal {
    pub fn open(&mut self, origin: re_uri::Origin, dataset_id: EntryId, asset_id: SegmentId) {
        self.state = Some(State {
            origin,
            dataset_id,
            asset_id,
        });
        self.modal.open();
    }

    pub fn ui(&mut self, ctx: &Context<'_>, ui: &egui::Ui) {
        let Some(state) = &self.state else {
            return;
        };

        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Unregister asset?"),
            |ui| {
                // The asset id gets its own line rather than being quoted into the sentence,
                // since it can be long.
                ui.strong(state.asset_id.as_str());
                ui.add_space(4.0);
                ui.label("This asset will be removed from the dataset.");

                ui.add_space(8.0);

                ui.label(
                    "The file it was registered from is left where it is, so you can register it \
                     again later. Segments that use the asset stop seeing it.",
                );

                ui.add_space(12.0);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Plain primary, since there is no destructive button variant yet.
                    let unregister = ui.add(ReButton::new("Unregister").primary().small());

                    if unregister.clicked() {
                        send_crossbeam(
                            ctx.command_sender,
                            Command::UnregisterAsset {
                                origin: state.origin.clone(),
                                entry_id: state.dataset_id,
                                asset_id: state.asset_id.clone(),
                                // A failed asset is unregistered without asking, so this modal
                                // only ever sees registered assets.
                                has_failed: false,
                            },
                        )
                        .ok();
                        ui.close();
                    }

                    // Cancel is the keyboard default, so both Enter and Escape close the modal.
                    if ui.add(ReButton::new("Cancel").small()).clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        ui.close();
                    }
                });
            },
        );

        if !self.modal.is_open() {
            self.state = None;
        }
    }
}
