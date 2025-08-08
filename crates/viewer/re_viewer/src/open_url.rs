//! Methods & structs for handling opening arbitrary URLs inside the viewer for both native and web.
//!
//! As opposed to opening them in a new tab.
//!
//! This is the highest level way of opening arbitrary URLs inside the viewer.
//! The only higher level way of opening URLs is `ui.ctx().open_url(...)` which will
//! open the URL in a browser if it's not a content URL that we can open inside the viewer.

use re_viewer_context::{CommandSender, Item, SystemCommand, SystemCommandSender as _};

/// A URL that points to a a selection (typically an entity) within the currently active recording.
pub const INTRA_RECORDING_URL_SCHEME: &str = "recording://";

/// An eventListener for rrd posted from containing html
pub const WEB_EVENT_LISTENER_SCHEME: &str = "web_event:";

/// Valid high level URLs types that can be opened inside the viewer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewerContentUrl {
    /// Could be a local path (`/foo.rrd`) or a remote url (`http://foo.com/bar.rrd`).
    ///
    /// Could be a link to either an `.rrd` recording or a `.rbl` blueprint.
    // TODO(andreas): no fragment support here? We used to have this.
    HttpRrd(String),

    /// gRPC Rerun Data Platform URL, e.g. `rerun://ip:port/recording/1234`
    RerunGrpcStream(re_uri::RedapUri),

    /// A URL that points to an entity within the currently active recording.
    ///
    /// String has [`INTRA_RECORDING_URL_SCHEME`] already stripped.
    /// (Selection may still be invalid at this point)
    IntraRecordingEntitySelection { selection: String },

    /// An eventListener for rrd posted from containing html
    ///
    /// Only available on the web viewer.
    WebEventListener(String),
}

impl std::fmt::Display for ViewerContentUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HttpRrd(url) => write!(f, "{}", url),
            Self::RerunGrpcStream(uri) => write!(f, "{}", uri),
            Self::IntraRecordingEntitySelection { selection } => {
                write!(f, "{INTRA_RECORDING_URL_SCHEME}{}", selection)
            }
            Self::WebEventListener(url) => write!(f, "{}", url),
        }
    }
}

impl ViewerContentUrl {
    /// Categorizes a URI into a [`CategorizedContentUrl`] if it's a URI that can be opened within the viewer.
    pub fn categorize_url(uri: String) -> Option<Self> {
        if let Ok(uri) = uri.parse::<re_uri::RedapUri>() {
            Some(Self::RerunGrpcStream(uri))
        } else if uri.starts_with(WEB_EVENT_LISTENER_SCHEME) {
            Some(Self::WebEventListener(uri))
        } else if (uri.starts_with("http") || uri.starts_with("https"))
            && (uri.ends_with(".rrd") || uri.ends_with(".rbl"))
        {
            Some(Self::HttpRrd(uri))
        } else if let Some(selection) = uri.strip_prefix(INTRA_RECORDING_URL_SCHEME) {
            Some(Self::IntraRecordingEntitySelection {
                selection: selection.to_owned(),
            })
        } else {
            // A non-rerun URL to something else entirely.
            // TODO(andreas): What about URLs that we could run through a data loader?
            None
        }
    }
}

/// Opens a URL in the viewer iff it's a valid content URL.
///
/// Will warn otherwise.
#[cfg(target_arch = "wasm32")]
pub fn try_open_url_in_viewer(
    egui_ctx: &egui::Context,
    follow_if_http: bool,
    select_redap_source_when_loaded: bool,
    url: String,
    command_sender: &CommandSender,
) {
    // TODO(andreas): Handle web viewer URLs gracefully. Document and describe the behavior for doing so.
    // See Rerun internal project issue https://linear.app/rerun/project/sharing-data-by-links-786c3809af14/overview#heading-link-sharing-between-native-and-web-4d5a5bb9

    if let Some(url) = ViewerContentUrl::categorize_url(url.clone()) {
        open_content_url_in_viewer(
            egui_ctx,
            follow_if_http,
            select_redap_source_when_loaded,
            url,
            command_sender,
        )
    } else {
        re_log::warn!("Failed to open {url:?} in the viewer: not a valid Rerun URL.");
    }
}

/// Opens an already categorized content URL inside the viewer.
pub fn open_content_url_in_viewer(
    egui_ctx: &egui::Context,
    follow_if_http: bool,
    select_redap_source_when_loaded: bool,
    endpoint: ViewerContentUrl,
    command_sender: &CommandSender,
) {
    re_log::debug!("Opening categorized URL: {endpoint:?}");

    // Most of everything we do in here is sending commands. Make sure we process them!
    egui_ctx.request_repaint();

    // For streaming in data spend a few more milliseconds decoding incoming messages,
    // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
    let ui_waker = {
        let egui_ctx = egui_ctx.clone();
        Box::new(move || {
            egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
        })
    };

    match endpoint {
        ViewerContentUrl::HttpRrd(url) => {
            let receiver = re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(
                url,
                follow_if_http,
                Some(ui_waker),
            );
            command_sender.send_system(SystemCommand::AddReceiver(receiver));
        }

        ViewerContentUrl::RerunGrpcStream(uri) => {
            re_log::debug!("Opening Rerun Grpc Stream: {uri:?}");

            command_sender.send_system(SystemCommand::LoadDataSource(
                re_data_source::DataSource::RerunGrpcStream {
                    uri,
                    select_when_loaded: select_redap_source_when_loaded,
                },
            ));
        }

        ViewerContentUrl::IntraRecordingEntitySelection { selection } => {
            match selection.parse::<Item>() {
                Ok(item) => {
                    command_sender.send_system(SystemCommand::SetSelection(item));
                }
                Err(err) => {
                    re_log::warn!("Failed to parse selection path {selection:?}: {err}");
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        ViewerContentUrl::WebEventListener(url) => {
            re_log::warn!(
                "Can't open {url}: {WEB_EVENT_LISTENER_SCHEME:?} urls are only available on the web viewer."
            );
        }

        #[cfg(target_arch = "wasm32")]
        ViewerContentUrl::WebEventListener(url) => {
            use std::{ops::ControlFlow, sync::Arc};

            // Process an rrd when it's posted via `window.postMessage`
            let (tx, rx) = re_smart_channel::smart_channel(
                re_smart_channel::SmartMessageSource::RrdWebEventCallback,
                re_smart_channel::SmartChannelSource::RrdWebEventListener,
            );
            re_log_encoding::stream_rrd_from_http::stream_rrd_from_event_listener(Arc::new({
                move |msg| {
                    ui_waker();
                    use re_log_encoding::stream_rrd_from_http::HttpMessage;
                    match msg {
                        HttpMessage::LogMsg(msg) => {
                            if tx.send(msg).is_ok() {
                                ControlFlow::Continue(())
                            } else {
                                re_log::info_once!(
                                    "Failed to send log message to viewer - closing connection to {url}"
                                );
                                ControlFlow::Break(())
                            }
                        }
                        HttpMessage::Success => {
                            tx.quit(None).warn_on_err_once("Failed to send quit marker");
                            ControlFlow::Break(())
                        }
                        HttpMessage::Failure(err) => {
                            tx.quit(Some(err))
                                .warn_on_err_once("Failed to send quit marker");
                            ControlFlow::Break(())
                        }
                    }
                }
            }));

            command_sender.send_system(SystemCommand::AddReceiver(rx));
        }
    }
}
