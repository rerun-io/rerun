//! A field for where a file is read from, which also takes a file picked on this machine.

use re_protos::capabilities::{
    CATALOG_WRITE_REGISTER, CATALOG_WRITE_STAGING, ServerCapabilities, catalog_write_register,
};
use re_redap_client::ConnectionRegistryHandle;
use re_ui::{UiExt as _, icons};

/// The kind of file a [`SourceField`] takes.
#[derive(Clone, Copy, Debug)]
pub struct FileKind {
    /// The name of the file dialog filter, like "Rerun recording".
    pub name: &'static str,

    /// The extension of the file, without the dot.
    pub extension: &'static str,

    /// The file name at the end of the example sources.
    pub example_file_name: &'static str,
}

/// What picking a file on this machine does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilePicker {
    /// The server reads the file where it is, through its `file://` uri.
    RegisterInPlace,

    /// The file is uploaded to the server's storage and registered from there.
    Upload,
}

/// A file picked on this machine.
pub struct PickedFile {
    /// The text the source field shows for the file. This is its `file://` uri on native and its
    /// name on web.
    pub label: String,

    #[cfg(not(target_arch = "wasm32"))]
    pub path: std::path::PathBuf,

    #[cfg(target_arch = "wasm32")]
    pub file: web_sys::File,
}

/// Picks files on this machine.
///
/// On web, [`Self::take_picked`] returns the file some frames after the dialog opens.
#[derive(Default)]
struct FileDialog {
    picked: std::sync::Arc<re_mutex::Mutex<Option<PickedFile>>>,
}

impl FileDialog {
    fn open(&self, egui_ctx: &egui::Context, kind: FileKind) {
        cfg_select! {
            target_arch = "wasm32" => {
                let picked = self.picked.clone();
                let egui_ctx = egui_ctx.clone();
                re_async::spawn_local(async move {
                    if let Some(file) = rfd::AsyncFileDialog::new()
                        .add_filter(kind.name, &[kind.extension])
                        .pick_file()
                        .await
                    {
                        *picked.lock() = Some(PickedFile {
                            label: file.file_name(),
                            file: file.inner().clone(),
                        });
                        egui_ctx.request_repaint();
                    }
                });
            }
            _ => {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter(kind.name, &[kind.extension])
                    .pick_file()
                else {
                    return;
                };

                let Ok(url) = url::Url::from_file_path(&path) else {
                    re_log::error!(
                        "Failed making a file uri out of the picked file\nFile path: {}",
                        path.display()
                    );
                    return;
                };

                *self.picked.lock() = Some(PickedFile {
                    label: url.to_string(),
                    path,
                });
                egui_ctx.request_repaint();
            }
        }
    }

    /// The file picked since the last call, if any.
    fn take_picked(&self) -> Option<PickedFile> {
        self.picked.lock().take()
    }
}

/// What a server takes as the source of a file it registers. A server that advertised no
/// capabilities lets every source through, for the server itself to answer.
#[derive(Clone, Debug)]
pub struct SourceCapabilities {
    capabilities: ServerCapabilities,

    /// Whether the server runs on this machine, in which case it reads the same filesystem as the
    /// viewer.
    is_localhost: bool,
}

impl SourceCapabilities {
    pub fn new(connection_registry: &ConnectionRegistryHandle, origin: &re_uri::Origin) -> Self {
        Self {
            capabilities: connection_registry.capabilities(origin),
            is_localhost: origin.is_localhost(),
        }
    }

    /// Whether the server registers anything at all.
    pub fn can_register(&self) -> bool {
        !self.capabilities.is_known()
            || self.capabilities.has_any_under(CATALOG_WRITE_REGISTER)
            || self.accepts_uploads()
    }

    /// The URL schemes the server reads a file from, empty if it did not say which.
    fn schemes(&self) -> Vec<&str> {
        self.capabilities.register_schemes()
    }

    /// Whether the server takes files uploaded to its storage.
    fn accepts_uploads(&self) -> bool {
        self.capabilities.has(CATALOG_WRITE_STAGING)
    }

    /// What picking a file on this machine does, if the field offers it.
    ///
    /// In the native viewer, a server on this machine that reads `file://` reads the file where it
    /// is. Any other server gets the file uploaded, if it takes uploads.
    fn file_picker(&self) -> Option<FilePicker> {
        // A browser gives the file contents, never a path the server could read.
        if cfg!(not(target_arch = "wasm32"))
            && self.is_localhost
            && self.schemes().contains(&"file")
        {
            Some(FilePicker::RegisterInPlace)
        } else if self.accepts_uploads() {
            Some(FilePicker::Upload)
        } else {
            None
        }
    }

    /// Why the server does not read a file from this source, if its scheme rules it out.
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
    fn source_hint(&self, kind: FileKind) -> String {
        let extension = kind.extension;
        let schemes = self.schemes();

        if schemes.is_empty() {
            format!("A .{extension} file the server can read.")
        } else {
            format!(
                "A .{extension} file the server can read, from {}.",
                list_schemes(&schemes)
            )
        }
    }

    /// An example source uri, in a scheme the server reads.
    pub fn source_example(&self, kind: FileKind) -> String {
        /// Shown for a server that did not say what it reads.
        const UNKNOWN_EXAMPLE: &str = "s3://bucket/path/";

        /// The start of an example per scheme, in the order this field picks them.
        const EXAMPLES: &[(&str, &str)] = &[
            ("file", "file:///path/to/"),
            ("s3", "s3://bucket/path/"),
            ("gs", "gs://bucket/path/"),
            ("az", "az://container/path/"),
            ("https", "https://example.com/path/"),
        ];

        let file_name = kind.example_file_name;
        let schemes = self.schemes();

        if let Some((_, example)) = EXAMPLES.iter().find(|(scheme, _)| schemes.contains(scheme)) {
            return format!("{example}{file_name}");
        }

        schemes.first().map_or_else(
            || format!("{UNKNOWN_EXAMPLE}{file_name}"),
            |scheme| format!("{scheme}://path/to/{file_name}"),
        )
    }
}

/// Where a file is read from, as a source uri the server reads or as a file picked on this
/// machine.
pub struct SourceField {
    kind: FileKind,

    /// The text in the field, as typed by the user or as the label of the picked file.
    text: String,

    /// The file to upload, as long as `text` still shows it.
    upload: Option<PickedFile>,

    file_dialog: FileDialog,
}

/// What a [`SourceField`] held.
pub enum Source {
    /// A source uri the server reads the file from.
    Uri(String),

    /// A file picked on this machine, to upload to the server's storage.
    Upload(PickedFile),
}

impl Source {
    /// The text the field showed for the source.
    pub fn label(&self) -> &str {
        match self {
            Self::Uri(uri) => uri,
            Self::Upload(file) => &file.label,
        }
    }
}

/// Whether the server takes what a [`SourceField`] holds.
enum SourceStatus {
    Valid,

    /// The text does not parse as a uri.
    InvalidUri,

    /// The server does not read the scheme of the uri, for this reason.
    RejectedScheme(String),
}

impl SourceField {
    pub fn new(kind: FileKind) -> Self {
        Self {
            kind,
            text: String::new(),
            upload: None,
            file_dialog: FileDialog::default(),
        }
    }

    /// The text in the field.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Empties the field and returns what it held.
    pub fn take(&mut self) -> Source {
        let text = std::mem::take(&mut self.text);

        match self.upload.take() {
            Some(file) => Source::Upload(file),
            None => Source::Uri(text),
        }
    }

    /// Shows the field with its label and the hints under it, and returns whether the server
    /// takes what the field holds.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        sources: &SourceCapabilities,
        request_focus: bool,
    ) -> bool {
        let file_picker = sources.file_picker();

        if let Some(picked) = self.file_dialog.take_picked() {
            self.text.clone_from(&picked.label);
            self.upload = (file_picker == Some(FilePicker::Upload)).then_some(picked);
        }

        // The field is outlined from the text it had before this frame's edits, so the outline lags
        // the text by one frame.
        let previous_source_is_invalid = !matches!(self.status(sources), SourceStatus::Valid);

        let label = ui.weak(egui::RichText::new("SOURCE URI").monospace().size(10.0));
        ui.add_space(4.0);

        let show_invalid = previous_source_is_invalid && !self.text.is_empty();
        let (field, picker_clicked) = source_uri_field(
            ui,
            &mut self.text,
            &sources.source_example(self.kind),
            file_picker,
            show_invalid,
        );
        let field = field.labelled_by(label.id);

        if request_focus {
            field.request_focus();
        }

        if picker_clicked {
            self.file_dialog.open(ui.ctx(), self.kind);
        }

        // Once the field no longer shows the picked file, the typed text is registered instead.
        self.upload.take_if(|upload| upload.label != self.text);

        // Parsed after the field, so the register button and what it sends both use the text the
        // user sees.
        let status = self.status(sources);

        ui.add_space(3.0);
        if self.upload.is_some() {
            ui.weak("The file is uploaded to the server's storage, and registered from there.");
        } else {
            ui.weak(sources.source_hint(self.kind));
            if file_picker == Some(FilePicker::Upload) {
                ui.weak("Or choose a file on this machine to upload it.");
            } else {
                ui.weak("There is no upload. The file must already be accessible to the server.");
            }
        }

        if let SourceStatus::RejectedScheme(reason) = &status {
            ui.add_space(4.0);
            ui.error_label(reason);
        }

        matches!(status, SourceStatus::Valid)
    }

    fn status(&self, sources: &SourceCapabilities) -> SourceStatus {
        if self.upload.is_some() {
            return SourceStatus::Valid;
        }

        match url::Url::parse(&self.text) {
            Ok(url) => match sources.scheme_rejection_reason(url.scheme()) {
                Some(reason) => SourceStatus::RejectedScheme(reason),
                None => SourceStatus::Valid,
            },
            Err(_) => SourceStatus::InvalidUri,
        }
    }
}

/// Where the file is read from, with a picker inside the field when the field takes a file on
/// this machine, and whether that picker was clicked.
///
/// This draws the frame instead of the [`egui::TextEdit`], so the picker sits inside it. An
/// `invalid` source is outlined in the error color, whatever the field's state.
fn source_uri_field(
    ui: &mut egui::Ui,
    source_uri: &mut String,
    hint_text: &str,
    picker: Option<FilePicker>,
    invalid: bool,
) -> (egui::Response, bool) {
    /// Tall enough for the picker, so the field does not change height with it.
    const FIELD_HEIGHT: f32 = 19.0;

    let textedit_id = ui.make_persistent_id("source_uri");
    let response = ui.read_response(textedit_id);

    let visuals = response
        .as_ref()
        .map(|response| ui.style().interact(response))
        .unwrap_or_else(|| &ui.visuals().widgets.inactive);

    let selection_stroke = ui.visuals().selection.stroke;
    let stroke = if invalid {
        egui::Stroke::new(selection_stroke.width, ui.visuals().error_fg_color)
    } else if response.is_some_and(|response| response.has_focus()) {
        selection_stroke
    } else {
        let mut stroke = visuals.bg_stroke;
        stroke.width = selection_stroke.width;
        stroke
    };

    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 9))
        .fill(ui.visuals().extreme_bg_color)
        .stroke(stroke)
        .corner_radius(9)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(FIELD_HEIGHT);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let picker_clicked = picker.is_some_and(|picker| {
                        let hover_text = match picker {
                            FilePicker::RegisterInPlace => "Choose a file",
                            FilePicker::Upload => "Choose a file to upload",
                        };
                        ui.small_icon_button(&icons::FOLDER, hover_text).clicked()
                    });

                    let response = ui.add(
                        egui::TextEdit::singleline(source_uri)
                            .id(textedit_id)
                            .font(egui::TextStyle::Monospace)
                            .frame(egui::Frame::new())
                            .hint_text(hint_text)
                            .desired_width(ui.available_width()),
                    );

                    (response, picker_clicked)
                })
                .inner
            })
            .inner
        })
        .inner
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

#[cfg(test)]
mod tests {
    use super::*;

    const RRD_FILE: FileKind = FileKind {
        name: "Rerun recording",
        extension: "rrd",
        example_file_name: "file.rrd",
    };

    /// A server on this machine that advertised it registers `schemes`.
    fn sources(schemes: &[&str]) -> SourceCapabilities {
        SourceCapabilities {
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
        let sources = SourceCapabilities {
            capabilities: ServerCapabilities::unknown(),
            is_localhost: true,
        };

        assert!(sources.can_register());
        assert!(sources.scheme_rejection_reason("s3").is_none());
        assert_eq!(sources.file_picker(), None);
    }

    /// A server that advertised capabilities but none for registering takes no source at all, and
    /// there is no scheme to offer the user.
    #[test]
    fn a_server_that_registers_nothing_takes_no_source() {
        let sources = sources(&[]);

        assert!(!sources.can_register());
        assert!(sources.schemes().is_empty());
        assert_eq!(sources.file_picker(), None);
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

    /// A server on this machine that reads `file://` registers a picked file where it is. A remote
    /// server reads its own filesystem rather than this one, so without uploads it offers no
    /// picker.
    #[test]
    fn only_a_local_file_reading_server_registers_a_picked_file_in_place() {
        assert_eq!(
            sources(&["file"]).file_picker(),
            Some(FilePicker::RegisterInPlace)
        );
        assert_eq!(
            sources(&["file", "s3"]).file_picker(),
            Some(FilePicker::RegisterInPlace)
        );
        assert_eq!(sources(&["s3"]).file_picker(), None);

        let remote = SourceCapabilities {
            is_localhost: false,
            ..sources(&["file"])
        };
        assert_eq!(remote.file_picker(), None);
    }

    /// A server that takes uploads gets a picked file uploaded, unless it runs on this machine and
    /// reads `file://`, in which case it reads the file where it is.
    #[test]
    fn a_server_that_takes_uploads_gets_a_picked_file_uploaded() {
        let uploading = |schemes: &[&str], is_localhost: bool| SourceCapabilities {
            capabilities: ServerCapabilities::from_advertised(std::iter::chain(
                schemes.iter().map(|scheme| catalog_write_register(scheme)),
                [CATALOG_WRITE_STAGING.to_owned()],
            )),
            is_localhost,
        };

        assert_eq!(
            uploading(&["s3"], false).file_picker(),
            Some(FilePicker::Upload)
        );
        assert_eq!(
            uploading(&["file"], false).file_picker(),
            Some(FilePicker::Upload)
        );
        assert_eq!(
            uploading(&["s3"], true).file_picker(),
            Some(FilePicker::Upload)
        );
        assert_eq!(
            uploading(&["file"], true).file_picker(),
            Some(FilePicker::RegisterInPlace)
        );
        assert!(uploading(&[], false).can_register());
    }

    /// The example source shown in the field and in the empty state is in a scheme the server
    /// reads, and names a file of the kind the field takes.
    #[test]
    fn the_example_source_is_in_a_scheme_the_server_reads() {
        assert_eq!(
            sources(&["file"]).source_example(RRD_FILE),
            "file:///path/to/file.rrd"
        );
        assert!(
            sources(&["s3", "https"])
                .source_example(RRD_FILE)
                .starts_with("s3://")
        );
        assert!(
            sources(&["ftp"])
                .source_example(RRD_FILE)
                .starts_with("ftp://")
        );
    }

    /// The hint under the field names what the server said it reads, and names no scheme at all
    /// when the server did not say.
    #[test]
    fn the_hint_names_only_the_advertised_schemes() {
        assert_eq!(
            sources(&["file", "s3"]).source_hint(RRD_FILE),
            "A .rrd file the server can read, from file:// or s3://."
        );

        let unknown = SourceCapabilities {
            capabilities: ServerCapabilities::unknown(),
            is_localhost: true,
        };
        assert_eq!(
            unknown.source_hint(RRD_FILE),
            "A .rrd file the server can read."
        );
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
}
