//! Registering a `.rrd` in object storage as an asset of a dataset.

use re_async::AsyncRuntimeHandle;
use re_format::{format_bytes, format_plural_s};
use re_log_types::EntryId;
use re_protos::common::v1alpha1::ext::{DatasetKind, DatasetLimits};
use re_quota_channel::send_crossbeam;
use re_redap_client::{
    AssetRegistrationError, ConnectionRegistryHandle, DEFAULT_ASSET_TASK_TIMEOUT, asset_data_source,
};
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{ReButton, UiExt as _};

use crate::context::Context;
use crate::servers::Command;

/// The dataset an asset is registered into.
#[derive(Clone, Debug)]
pub struct AssetTarget {
    pub origin: re_uri::Origin,

    pub dataset_id: EntryId,
}

/// How full the asset slots of a dataset are, as of this frame.
#[derive(Clone, Debug, Default)]
pub struct AssetSlots {
    /// How many assets the dataset holds.
    pub registered_count: usize,

    /// The source uris the server is already registering with the dataset.
    pub pending_source_uris: Vec<String>,
}

impl AssetSlots {
    /// How many asset slots of the dataset are taken.
    ///
    /// A pending registration has taken its slot even though the server has yet to answer.
    pub fn taken(&self) -> usize {
        self.registered_count + self.pending_source_uris.len()
    }

    /// How many more assets the dataset accepts, if their number is limited.
    fn left(&self) -> Option<usize> {
        DatasetKind::Asset
            .limits()
            .max_segment_count
            .map(|max_count| (max_count as usize).saturating_sub(self.taken()))
    }

    /// Why the dataset cannot take another asset, if it cannot.
    pub fn no_room_reason(&self) -> Option<String> {
        let max_count = DatasetKind::Asset.limits().max_segment_count?;

        if self.left() != Some(0) {
            return None;
        }

        let pending = self.pending_source_uris.len();

        Some(if pending == 0 {
            format!(
                "This dataset holds all {max_count} assets it is allowed. Unregister one to make \
                 room."
            )
        } else {
            format!(
                "This dataset holds all {max_count} assets it is allowed, counting {} still \
                 running. Unregister one to make room.",
                format_plural_s(pending, "registration"),
            )
        })
    }

    /// Whether the server is already registering `source_uri` with the dataset.
    pub fn is_registering(&self, source_uri: &str) -> bool {
        self.pending_source_uris
            .iter()
            .any(|pending| pending == source_uri)
    }

    /// The rules the server applies to the assets of a dataset, one per rule.
    ///
    /// The limit on their number shows as how many slots are left.
    pub fn limit_rules(&self) -> Vec<String> {
        // TODO(isse): Get this from the server?
        let DatasetLimits {
            static_chunks_only,
            max_segment_size_bytes,
            max_segment_count,
        } = DatasetKind::Asset.limits();

        let mut rules = Vec::new();

        if static_chunks_only {
            rules.push("static data only".to_owned());
        }

        if let Some(max_size) = max_segment_size_bytes {
            rules.push(format!("≤ {} per asset", format_bytes(max_size as f64)));
        }

        if let Some(max_count) = max_segment_count {
            let left = self.left().unwrap_or(0);
            rules.push(format!("{left} of {max_count} slots left"));
        }

        rules
    }
}

/// Modal for registering an asset with a dataset.
#[derive(Default)]
pub struct RegisterAssetModal {
    modal: ModalHandler,

    state: Option<State>,

    just_opened: bool,
}

/// What the modal works on while it is open.
struct State {
    target: AssetTarget,
    connection_registry: ConnectionRegistryHandle,
    runtime: AsyncRuntimeHandle,

    /// Where the asset is read from, as typed by the user.
    source_uri: String,
}

impl RegisterAssetModal {
    pub fn open(
        &mut self,
        target: AssetTarget,
        connection_registry: ConnectionRegistryHandle,
        runtime: AsyncRuntimeHandle,
    ) {
        self.state = Some(State {
            target,
            connection_registry,
            runtime,
            source_uri: String::new(),
        });
        self.just_opened = true;
        self.modal.open();
    }

    /// The dataset the modal registers into, while it is open.
    pub fn target(&self) -> Option<&AssetTarget> {
        self.state.as_ref().map(|state| &state.target)
    }

    pub fn ui(&mut self, ctx: &Context<'_>, ui: &egui::Ui, slots: &AssetSlots) {
        let Some(state) = &mut self.state else {
            return;
        };
        let just_opened = std::mem::take(&mut self.just_opened);

        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Register an asset"),
            |ui| {
                // Parsed before the field is drawn, so the outline and the register button lag the
                // text by one frame.
                let source = asset_data_source(&state.target.origin, &state.source_uri);

                let label = ui.monospace("SOURCE URI");

                ui.scope(|ui| {
                    if source.is_err() && !state.source_uri.is_empty() {
                        ui.style_invalid_field();
                    }

                    let field = ui
                        .add(
                            egui::TextEdit::singleline(&mut state.source_uri)
                                .hint_text("s3://bucket/path/asset.rrd")
                                .desired_width(f32::INFINITY),
                        )
                        .labelled_by(label.id);

                    if just_opened {
                        field.request_focus();
                    }
                });

                ui.weak(
                    "A .rrd file the server can read. For a Rerun Hub server, that can be s3://, az:// or https://. For a local server use file://<file path>",
                );

                ui.add_space(4.0);
                for rule in slots.limit_rules() {
                    ui.horizontal(|ui| {
                        ui.bullet(ui.tokens().text_subdued);
                        ui.weak(rule);
                    });
                }

                let no_room_reason = slots.no_room_reason();
                if let Some(reason) = &no_room_reason {
                    ui.add_space(4.0);
                    ui.warning_label(reason);
                }

                let already_registering = slots.is_registering(&state.source_uri);
                if already_registering {
                    ui.add_space(4.0);
                    ui.warning_label("The server is already registering this asset.");
                }

                ui.add_space(4.0);
                ui.weak(
                    "Registering runs in the background. \
                     The asset list shows how far the server got with it.",
                );

                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_register =
                        source.is_ok() && no_room_reason.is_none() && !already_registering;

                    let register_response =
                        ui.add_enabled(can_register, ReButton::new("Register").blue().small());

                    if can_register
                        && (register_response.clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        let AssetTarget { origin, dataset_id } = state.target.clone();
                        let connection_registry = state.connection_registry.clone();
                        let source_uri = std::mem::take(&mut state.source_uri);
                        let command_sender = ctx.command_sender.clone();
                        let egui_ctx = ui.ctx().clone();

                        send_crossbeam(
                            &command_sender,
                            Command::AssetRegistrationStarted {
                                origin: origin.clone(),
                                entry_id: dataset_id,
                                source_uri: source_uri.clone(),
                            },
                        )
                        .ok();

                        state.runtime.spawn_future(async move {
                            let result = register_asset(
                                connection_registry,
                                origin.clone(),
                                dataset_id,
                                &source_uri,
                            )
                            .await;

                            send_crossbeam(
                                &command_sender,
                                Command::AssetRegistrationFinished {
                                    origin,
                                    entry_id: dataset_id,
                                    source_uri,
                                    result,
                                },
                            )
                            .ok();
                            egui_ctx.request_repaint();
                        });

                        ui.close();
                    }

                    if ui.add(ReButton::new("Cancel").small()).clicked() {
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

/// Registers what `source_uri` points at as an asset of a dataset.
async fn register_asset(
    connection_registry: ConnectionRegistryHandle,
    origin: re_uri::Origin,
    dataset_id: EntryId,
    source_uri: &str,
) -> Result<(), AssetRegistrationError> {
    match connection_registry
        .connection_handle(origin)
        .register_asset(dataset_id, source_uri, DEFAULT_ASSET_TASK_TIMEOUT)
        .await
    {
        Ok(asset_id) => {
            re_log::info!("Successfully registered asset '{source_uri}' as '{asset_id}'");
            Ok(())
        }
        Err(err) => {
            re_log::error!("Failed registering asset '{source_uri}': {err}");
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many assets a dataset is allowed to hold.
    fn max_count() -> usize {
        DatasetKind::Asset
            .limits()
            .max_segment_count
            .expect("asset datasets limit how many assets they hold") as usize
    }

    fn slots(registered_count: usize, pending: usize) -> AssetSlots {
        AssetSlots {
            registered_count,
            pending_source_uris: (0..pending)
                .map(|idx| format!("s3://bucket/pending-{idx}.rrd"))
                .collect(),
        }
    }

    /// A dataset with a slot to spare takes another asset, and one holding all the assets it is
    /// allowed says why it cannot.
    #[test]
    fn a_full_dataset_has_no_room_for_another_asset() {
        assert_eq!(slots(max_count() - 1, 0).left(), Some(1));
        assert!(slots(max_count() - 1, 0).no_room_reason().is_none());

        assert_eq!(slots(max_count(), 0).left(), Some(0));
        assert!(slots(max_count(), 0).no_room_reason().is_some());
    }

    /// A registration the server is still working on holds its slot, so it fills up a dataset the
    /// same way a registered asset does.
    #[test]
    fn a_pending_registration_takes_a_slot() {
        let slots = slots(max_count() - 1, 1);

        assert_eq!(slots.taken(), max_count());
        assert_eq!(slots.left(), Some(0));
        assert!(slots.no_room_reason().is_some());
        assert!(slots.is_registering("s3://bucket/pending-0.rrd"));
        assert!(!slots.is_registering("s3://bucket/other.rrd"));
    }
}
