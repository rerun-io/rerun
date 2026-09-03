//! Registering a `.rrd` in object storage as an asset of a dataset.

use re_async::AsyncRuntimeHandle;
use re_format::{format_bytes, format_plural_s};
use re_log_types::EntryId;
use re_protos::capabilities::{CATALOG_WRITE_REGISTER, ServerCapabilities, catalog_write_register};
use re_protos::common::v1alpha1::ext::{DatasetKind, DatasetLimits};
use re_quota_channel::send_crossbeam;
use re_redap_client::{
    AssetRegistrationError, ConnectionRegistryHandle, DEFAULT_ASSET_TASK_TIMEOUT, asset_data_source,
};
use re_ui::modal::{ModalHandler, ModalWrapper};
use re_ui::{ReButton, UiExt as _, icons};

use crate::context::Context;
use crate::servers::Command;

/// The dataset an asset is registered into.
#[derive(Clone, Debug)]
pub struct AssetTarget {
    pub origin: re_uri::Origin,

    pub dataset_id: EntryId,
}

/// What a server takes as the source of an asset. A server that advertised no capabilities
/// lets every source through, for the server itself to answer.
#[derive(Clone, Debug)]
pub struct AssetSourcesCapabilities {
    capabilities: ServerCapabilities,

    /// Whether the server runs on this machine, so we can assume reads the same filesystem
    /// the viewer does.
    is_localhost: bool,
}

impl AssetSourcesCapabilities {
    pub fn new(connection_registry: &ConnectionRegistryHandle, origin: &re_uri::Origin) -> Self {
        Self {
            capabilities: connection_registry.capabilities(origin),
            is_localhost: origin.is_localhost(),
        }
    }

    /// Whether the server takes any asset at all.
    pub fn registers_assets(&self) -> bool {
        !self.capabilities.is_known() || self.capabilities.has_any_under(CATALOG_WRITE_REGISTER)
    }

    /// The URL schemes the server reads an asset from, empty if it did not say which.
    fn schemes(&self) -> Vec<&str> {
        self.capabilities.register_schemes()
    }

    /// Whether the file to register can be picked off this machine: the server runs here, and it
    /// reads `file://`.
    fn show_file_picker(&self) -> bool {
        // Web doesn't have access to the same file system.
        cfg!(not(target_arch = "wasm32")) && self.is_localhost && self.schemes().contains(&"file")
    }

    /// Why the server does not read an asset from this source, if its scheme rules it out.
    fn scheme_rejection_reason(&self, scheme: &str) -> Option<String> {
        if self.capabilities.has(&catalog_write_register(scheme)) {
            return None;
        }

        let schemes = self.schemes();
        if schemes.is_empty() {
            return None;
        }

        Some(format!(
            "This server does not read {scheme}:// sources. It reads {}.",
            list_schemes(&schemes)
        ))
    }

    /// What the source field asks for, naming the schemes the server reads. A server that did not
    /// say which reads no scheme in particular, so none is named.
    fn source_hint(&self) -> String {
        let schemes = self.schemes();

        if schemes.is_empty() {
            "A .rrd file the server can read.".to_owned()
        } else {
            format!(
                "A .rrd file the server can read, from {}.",
                list_schemes(&schemes)
            )
        }
    }

    /// An example source uri, in a scheme the server reads.
    pub fn source_example(&self) -> String {
        /// Shown for a server that did not say what it reads.
        const UNKNOWN_EXAMPLE: &str = "s3://bucket/path/asset.rrd";

        /// An example per scheme, in the order this field picks them.
        const EXAMPLES: &[(&str, &str)] = &[
            ("file", "file:///path/to/file.rrd"),
            ("s3", "s3://bucket/path/asset.rrd"),
            ("gs", "gs://bucket/path/asset.rrd"),
            ("az", "az://container/path/asset.rrd"),
            ("https", "https://example.com/path/asset.rrd"),
        ];

        let schemes = self.schemes();

        if let Some((_, example)) = EXAMPLES.iter().find(|(scheme, _)| schemes.contains(scheme)) {
            return (*example).to_owned();
        }

        schemes.first().map_or_else(
            || UNKNOWN_EXAMPLE.to_owned(),
            |scheme| format!("{scheme}://path/to/asset.rrd"),
        )
    }
}

/// Where the asset is read from, with a picker inside the field when the server reads a file off
/// this machine.
///
/// The frame is drawn here rather than by the [`egui::TextEdit`], so that the picker sits inside
/// it. Its stroke follows the field the way `re_ui`'s search field does, which keeps
/// [`re_ui::UiExt::style_invalid_field`] working.
fn source_uri_field(
    ui: &mut egui::Ui,
    source_uri: &mut String,
    hint_text: &str,
    with_picker: bool,
) -> egui::Response {
    /// Tall enough for the picker, so the field does not change height with it.
    const FIELD_HEIGHT: f32 = 19.0;

    let textedit_id = ui.id().with("source_uri");
    let response = ui.read_response(textedit_id);

    let visuals = response
        .as_ref()
        .map(|response| ui.style().interact(response))
        .unwrap_or_else(|| &ui.visuals().widgets.inactive);

    let selection_stroke = ui.visuals().selection.stroke;
    let stroke = if response.is_some_and(|response| response.has_focus()) {
        selection_stroke
    } else {
        let mut stroke = visuals.bg_stroke;
        stroke.width = selection_stroke.width;
        stroke
    };

    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(3, 2))
        .fill(ui.visuals().extreme_bg_color)
        .stroke(stroke)
        .corner_radius(visuals.corner_radius)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(FIELD_HEIGHT);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if with_picker && let Some(picked) = pick_asset_file(ui) {
                        *source_uri = picked;
                    }

                    ui.add(
                        egui::TextEdit::singleline(source_uri)
                            .id(textedit_id)
                            .frame(egui::Frame::new())
                            .hint_text(hint_text)
                            .desired_width(ui.available_width()),
                    )
                })
                .inner
            })
            .inner
        })
        .inner
}

/// A button that picks a `.rrd` off this machine, and the `file://` uri it picked.
fn pick_asset_file(ui: &mut egui::Ui) -> Option<String> {
    let clicked = ui
        .small_icon_button(&icons::FOLDER, "Choose a file")
        .clicked();

    cfg_select! {
        // The wasm server reads its files from OPFS and registers none of them, so this button
        // never shows up there.
        target_arch = "wasm32" => {
            let _ = clicked;
            None
        }
        _ => {
            if !clicked {
                return None;
            }

            let path = rfd::FileDialog::new()
                .add_filter("Rerun recording", &["rrd"])
                .pick_file()?;

            let Ok(url) = url::Url::from_file_path(&path) else {
                re_log::error!(
                    "Failed making a file uri out of the picked file\nFile path: {}",
                    path.display()
                );
                return None;
            };

            Some(url.to_string())
        }
    }
}

/// Names the schemes as `file://`, `s3://` or `https://`.
fn list_schemes(schemes: &[&str]) -> String {
    let named: Vec<String> = schemes
        .iter()
        .map(|scheme| format!("{scheme}://"))
        .collect();

    match named.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    }
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
                // Read every frame, so the modal keeps up with a connection made while it is open.
                let sources =
                    AssetSourcesCapabilities::new(&state.connection_registry, &state.target.origin);

                // `style_invalid_field` has to be applied before the field is added, so the outline
                // lags the text by one frame.
                let previous_source_is_invalid =
                    match asset_data_source(&state.target.origin, &state.source_uri) {
                        Ok(source) => sources
                            .scheme_rejection_reason(source.storage_url.scheme())
                            .is_some(),
                        Err(_) => true,
                    };

                let label = ui.strong("Source URI");

                ui.scope(|ui| {
                    if previous_source_is_invalid && !state.source_uri.is_empty() {
                        ui.style_invalid_field();
                    }

                    let field = source_uri_field(
                        ui,
                        &mut state.source_uri,
                        &sources.source_example(),
                        sources.show_file_picker(),
                    )
                    .labelled_by(label.id);

                    if just_opened {
                        field.request_focus();
                    }
                });

                // Parsed after the field, so the register button and what it sends both use the
                // text the user sees.
                let source = asset_data_source(&state.target.origin, &state.source_uri);
                let rejected_scheme_reason = source.as_ref().ok().and_then(|source| {
                    sources.scheme_rejection_reason(source.storage_url.scheme())
                });

                ui.label(sources.source_hint());

                if let Some(reason) = &rejected_scheme_reason {
                    ui.add_space(4.0);
                    ui.error_label(reason);
                }

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
                ui.label(
                    "Registering runs in the background. \
                     The asset list shows how far the server got with it.",
                );

                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_register = source.is_ok()
                        && rejected_scheme_reason.is_none()
                        && no_room_reason.is_none()
                        && !already_registering;

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

    /// A server on this machine that advertised it registers `schemes`.
    fn sources(schemes: &[&str]) -> AssetSourcesCapabilities {
        AssetSourcesCapabilities {
            capabilities: ServerCapabilities::from_advertised(
                schemes.iter().map(|scheme| catalog_write_register(scheme)),
            ),
            is_localhost: true,
        }
    }

    /// A server from before capabilities existed says nothing about what it takes, so every source
    /// is let through for the server itself to answer.
    #[test]
    fn a_server_that_advertised_nothing_takes_any_source() {
        let sources = AssetSourcesCapabilities {
            capabilities: ServerCapabilities::unknown(),
            is_localhost: true,
        };

        assert!(sources.registers_assets());
        assert!(sources.scheme_rejection_reason("s3").is_none());
        assert!(!sources.show_file_picker());
    }

    /// A server that advertised capabilities but none for registering takes no asset at all, and
    /// there is no scheme to offer the user.
    #[test]
    fn a_server_that_registers_nothing_takes_no_source() {
        let sources = sources(&[]);

        assert!(!sources.registers_assets());
        assert!(sources.schemes().is_empty());
        assert!(!sources.show_file_picker());
    }

    /// A source in a scheme the server did not advertise is refused before it is sent, whatever
    /// the scheme is, and the reason names what the server does read.
    #[test]
    fn a_scheme_the_server_does_not_read_is_refused() {
        let sources = sources(&["file"]);

        assert_eq!(sources.scheme_rejection_reason("file"), None);
        assert_eq!(
            sources.scheme_rejection_reason("ftp").as_deref(),
            Some("This server does not read ftp:// sources. It reads file://.")
        );
        assert_eq!(
            sources.scheme_rejection_reason("s3").as_deref(),
            Some("This server does not read s3:// sources. It reads file://.")
        );
    }

    /// The picker only makes sense when the file to register is one this machine can point at. A
    /// server on this machine offers it as long as it reads `file://`. A remote server reads its
    /// own filesystem rather than this one, so it never offers it.
    #[test]
    fn only_a_local_file_reading_server_offers_the_picker() {
        assert!(sources(&["file"]).show_file_picker());
        assert!(sources(&["file", "s3"]).show_file_picker());
        assert!(!sources(&["s3"]).show_file_picker());

        let remote = AssetSourcesCapabilities {
            is_localhost: false,
            ..sources(&["file"])
        };
        assert!(!remote.show_file_picker());
    }

    /// The example source shown in the field and in the empty state is in a scheme the server
    /// actually reads.
    #[test]
    fn the_example_source_is_in_a_scheme_the_server_reads() {
        assert!(sources(&["file"]).source_example().starts_with("file://"));
        assert!(
            sources(&["s3", "https"])
                .source_example()
                .starts_with("s3://")
        );
        assert!(sources(&["ftp"]).source_example().starts_with("ftp://"));
    }

    /// The hint under the field names what the server said it reads, and names no scheme at all
    /// when the server did not say.
    #[test]
    fn the_hint_names_only_the_advertised_schemes() {
        assert_eq!(
            sources(&["file", "s3"]).source_hint(),
            "A .rrd file the server can read, from file:// or s3://."
        );

        let unknown = AssetSourcesCapabilities {
            capabilities: ServerCapabilities::unknown(),
            is_localhost: true,
        };
        assert_eq!(unknown.source_hint(), "A .rrd file the server can read.");
    }

    /// The schemes are named so they read as a sentence, however many there are.
    #[test]
    fn schemes_are_named_as_a_list() {
        assert_eq!(list_schemes(&["file"]), "file://");
        assert_eq!(list_schemes(&["file", "s3"]), "file:// or s3://");
        assert_eq!(
            list_schemes(&["file", "s3", "https"]),
            "file://, s3:// or https://"
        );
    }

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
