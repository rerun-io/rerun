//! Registering a `.rrd` in object storage as an asset of a dataset.

use re_async::AsyncRuntimeHandle;
use re_catalog_client::{
    AssetCoverage, AssetRegistrationError, CatalogClient, DEFAULT_ASSET_TASK_TIMEOUT,
};
use re_format::{format_bytes, format_plural_s};
use re_log_types::EntryId;
use re_protos::common::v1alpha1::ext::{DatasetKind, DatasetLimits};
use re_quota_channel::send_crossbeam;
use re_redap_client::ConnectionRegistryHandle;
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{ButtonExt as _, UiExt as _};

use crate::asset_coverage_modal::{
    AssetEditState, AssetTarget, COVERAGE_MODAL_WIDTH, COVERAGE_NOT_STORED, assets_docs_link,
    modal_body_max_height, remember_footer_height,
};
use crate::context::Context;
use crate::entry_meta::DatasetSegments;
use crate::servers::Command;
use crate::source_field::{FileKind, Source, SourceCapabilities, SourceField};
use crate::upload_asset::UploadAssetExt as _;

/// An asset is a single `.rrd`.
pub const ASSET_FILE: FileKind = FileKind {
    name: "Rerun recording",
    extension: "rrd",
    example_file_name: "asset.rrd",
};

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
    edit_state: AssetEditState,

    /// Where the asset is read from.
    source: SourceField,

    /// Whether registering leaves the coverage the server stores for the asset as it is.
    keep_coverage: bool,
}

impl RegisterAssetModal {
    pub fn open(
        &mut self,
        target: AssetTarget,
        connection_registry: &ConnectionRegistryHandle,
        runtime: AsyncRuntimeHandle,
    ) {
        self.state = Some(State {
            edit_state: AssetEditState {
                client: CatalogClient::new(
                    connection_registry.connection_handle(target.origin.clone()),
                    runtime,
                ),
                target,
                coverage: Default::default(),
            },
            source: SourceField::new(ASSET_FILE),
            keep_coverage: true,
        });
        self.just_opened = true;
        self.modal.open();
    }

    /// The dataset the modal registers into, while it is open.
    pub fn target(&self) -> Option<&AssetTarget> {
        self.state.as_ref().map(|state| &state.edit_state.target)
    }

    /// `dataset_segments` are the segments the coverage cards name and search.
    ///
    /// Without `stores_coverage` the modal offers no coverage, and the asset applies to every
    /// segment.
    pub fn ui(
        &mut self,
        ctx: &Context<'_>,
        ui: &egui::Ui,
        slots: &AssetSlots,
        dataset_segments: &DatasetSegments,
        stores_coverage: bool,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        let just_opened = std::mem::take(&mut self.just_opened);

        self.modal.ui(
            ui.ctx(),
            || {
                ModalWrapper::new("Register an asset")
                    .max_width(COVERAGE_MODAL_WIDTH)
                    .max_height(f32::INFINITY)
            },
            |ui| {
                // Read every frame, so the modal keeps up with a connection made while it is open.
                let sources = SourceCapabilities::new(
                    state.edit_state.client.connection().connection_registry(),
                    &state.edit_state.target.origin,
                );

                let footer_id = ui.make_persistent_id("register_asset_footer");

                let source_is_valid = egui::ScrollArea::vertical()
                    .id_salt("register_asset_body")
                    .max_height(modal_body_max_height(ui, footer_id))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add_space(10.0);
                        let source_is_valid = state.source.ui(ui, &sources, just_opened);

                        ui.add_space(10.0);
                        for rule in slots.limit_rules() {
                            ui.horizontal(|ui| {
                                ui.bullet(ui.tokens().text_subdued);
                                ui.weak(rule);
                            });
                        }

                        if let Some(reason) = &slots.no_room_reason() {
                            ui.add_space(4.0);
                            ui.warning_label(reason);
                        }

                        ui.add_space(14.0);
                        egui::Sides::new().show(
                            ui,
                            |ui| {
                                ui.weak(egui::RichText::new("APPLIES TO").monospace().size(10.0));
                            },
                            |ui| {
                                if stores_coverage {
                                    ui.checkbox(
                                        &mut state.keep_coverage,
                                        "Keep current coverage",
                                    )
                                    .on_hover_text(
                                        "An asset that is already registered keeps the coverage it \
                                         has. A new asset applies to every segment.",
                                    );
                                }
                            },
                        );
                        ui.add_space(4.0);

                        if stores_coverage {
                            let keep_coverage = state.keep_coverage;
                            ui.add_enabled_ui(!keep_coverage, |ui| {
                                state.edit_state.coverage.ui(ui, dataset_segments, 50.0);
                            });
                        } else {
                            ui.weak(COVERAGE_NOT_STORED);
                        }

                        if slots.is_registering(state.source.text()) {
                            ui.add_space(4.0);
                            ui.warning_label("The server is already registering this asset.");
                        }

                        ui.add_space(10.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.weak(
                                "Registration runs in the background. \
                                 The asset appears as pending until the server validates it.",
                            );
                            assets_docs_link(ui);
                        });

                        source_is_valid
                    })
                    .inner;

                let no_room_reason = slots.no_room_reason();
                let keep_coverage = state.keep_coverage || !stores_coverage;
                let already_registering = slots.is_registering(state.source.text());

                let footer = ui.scope(|ui| {
                    ui.add_space(16.0);
                    egui::Sides::new().shrink_left().truncate().show(
                        ui,
                        |ui| {
                            if !keep_coverage {
                                state.edit_state.coverage.summary_ui(ui, dataset_segments);
                            }
                        },
                        |ui| {
                            let can_register =
                                source_is_valid && no_room_reason.is_none() && !already_registering;

                            let register_response = ui.add_enabled(
                                can_register,
                                egui::Button::new("Register").blue().size_small(),
                            );

                            if can_register
                                && (register_response.clicked()
                                    || ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                let AssetTarget { origin, dataset_id } =
                                    state.edit_state.target.clone();
                                let client = state.edit_state.client.clone();
                                let coverage =
                                    (!keep_coverage).then(|| state.edit_state.coverage.coverage());
                                let source = state.source.take();
                                let source_uri = source.label().to_owned();
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

                                state.edit_state.client.runtime().spawn_future(async move {
                                    let result = register_asset(
                                        client,
                                        dataset_id,
                                        source,
                                        coverage.as_ref(),
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

                            if ui.add(egui::Button::new("Cancel").size_small()).clicked() {
                                ui.close();
                            }
                        },
                    );
                });
                remember_footer_height(ui, footer_id, footer.response.rect);
            },
        );

        if !self.modal.is_open() {
            self.state = None;
        }
    }
}

/// Registers `source` as an asset of a dataset, covering the segments the user picked.
///
/// A [`Source::Upload`] is uploaded to the server's storage first.
async fn register_asset(
    client: CatalogClient,
    dataset_id: EntryId,
    source: Source,
    coverage: Option<&AssetCoverage>,
) -> Result<(), AssetRegistrationError> {
    let source_uri = source.label().to_owned();

    let registered = match source {
        Source::Uri(uri) => {
            client
                .register_asset(dataset_id, &uri, coverage, DEFAULT_ASSET_TASK_TIMEOUT)
                .await
        }
        Source::Upload(upload) => {
            cfg_select! {
                target_arch = "wasm32" => {
                    client
                        .upload_asset(
                            dataset_id,
                            re_web::fs::File::from(upload.file),
                            coverage,
                            DEFAULT_ASSET_TASK_TIMEOUT,
                        )
                        .await
                }
                _ => {
                    client
                        .upload_asset_path(
                            dataset_id,
                            upload.path,
                            coverage,
                            DEFAULT_ASSET_TASK_TIMEOUT,
                        )
                        .await
                }
            }
        }
    };

    match registered {
        Ok(asset_id) => {
            re_log::info!("Registered asset as '{asset_id}'\nSource: {source_uri}");
            Ok(())
        }
        Err(err) => {
            re_log::error!("Failed registering asset: {err}\nSource: {source_uri}");
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
