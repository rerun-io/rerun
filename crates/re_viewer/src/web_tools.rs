use std::{ops::ControlFlow, sync::Arc};

use anyhow::Context as _;
use wasm_bindgen::JsValue;

use re_log::ResultExt as _;
use re_viewer_context::CommandSender;

/// Web-specific tools used by various parts of the application.

pub fn set_url_parameter_and_refresh(key: &str, value: &str) -> Result<(), wasm_bindgen::JsValue> {
    let Some(window) = web_sys::window() else {
        return Err("Failed to get window".into());
    };
    let location = window.location();

    let url = web_sys::Url::new(&location.href()?)?;
    url.search_params().set(key, value);

    location.assign(&url.href())
}

/// Percent-encode the given string so you can put it in a URL.
pub fn percent_encode(s: &str) -> String {
    format!("{}", js_sys::encode_uri_component(s))
}

/// Push a relative url on the web `History`,
/// so that the user can use the back button to navigate to it.
///
/// If this is already the current url, nothing happens.
///
/// The url must be percent encoded.
///
/// Example:
/// ```
/// push_history("foo/bar?baz=qux#fragment");
/// ```
pub fn push_history(new_relative_url: &str) -> Option<()> {
    let location = web_sys::window()?.location();

    let search = location.search().unwrap_or_default();
    let hash = location.hash().unwrap_or_default();
    let current_relative_url = format!("{search}{hash}");

    if current_relative_url == new_relative_url {
        re_log::debug!("Ignoring navigation to {new_relative_url:?} as we're already there");
    } else {
        re_log::debug!(
            "Existing url is {current_relative_url:?}; navigating to {new_relative_url:?}"
        );

        let history = web_sys::window()?
            .history()
            .map_err(|err| format!("Failed to get History API: {err:?}"))
            .ok_or_log_error()?;
        history
            .push_state_with_url(&JsValue::NULL, "", Some(new_relative_url))
            .map_err(|err| format!("Failed to push history state: {err:?}"))
            .ok_or_log_error()?;
    }
    Some(())
}

/// Parse the `?query` parst of the url, and translate it into commands to control the application.
pub fn translate_query_into_commands(egui_ctx: &egui::Context, command_sender: &CommandSender) {
    use re_viewer_context::{SystemCommand, SystemCommandSender as _};

    let location = eframe::web::web_location();

    // NOTE: it's unclear what to do if we find bout `examples` and `url` in the query.

    if location.query_map.get("examples").is_some() {
        command_sender.send_system(SystemCommand::CloseAllRecordings);
    }

    // NOTE: we support passing in multiple urls to multiple different recorording, blueprints, etc
    let urls: Vec<&String> = location
        .query_map
        .get("url")
        .into_iter()
        .flatten()
        .collect();
    if !urls.is_empty() {
        // Clear out any already open recordings to make room for the new ones.
        command_sender.send_system(SystemCommand::CloseAllRecordings);

        for url in urls {
            if let Some(receiver) = url_to_receiver(egui_ctx.clone(), url).ok_or_log_error() {
                command_sender.send_system(SystemCommand::AddReceiver(receiver));
            }
        }
    }

    egui_ctx.request_repaint(); // wake up to receive the messages
}

enum EndpointCategory {
    /// Could be a local path (`/foo.rrd`) or a remote url (`http://foo.com/bar.rrd`).
    ///
    /// Could be a link to either an `.rrd` recording or a `.rbl` blueprint.
    HttpRrd(String),

    /// A remote Rerun server.
    WebSocket(String),

    /// An eventListener for rrd posted from containing html
    WebEventListener,
}

impl EndpointCategory {
    fn categorize_uri(uri: &str) -> Self {
        if uri.starts_with("http") || uri.ends_with(".rrd") || uri.ends_with(".rbl") {
            Self::HttpRrd(uri.into())
        } else if uri.starts_with("ws:") || uri.starts_with("wss:") {
            Self::WebSocket(uri.into())
        } else if uri.starts_with("web_event:") {
            Self::WebEventListener
        } else {
            // If this is something like `foo.com` we can't know what it is until we connect to it.
            // We could/should connect and see what it is, but for now we just take a wild guess instead:
            re_log::info!("Assuming WebSocket endpoint");
            if uri.contains("://") {
                Self::WebSocket(uri.into())
            } else {
                Self::WebSocket(format!("{}://{uri}", re_ws_comms::PROTOCOL))
            }
        }
    }
}

/// Start receiving from the given url.
pub fn url_to_receiver(
    egui_ctx: egui::Context,
    url: &str,
) -> anyhow::Result<re_smart_channel::Receiver<re_log_types::LogMsg>> {
    let ui_waker = Box::new(move || {
        // Spend a few more milliseconds decoding incoming messages,
        // then trigger a repaint (https://github.com/rerun-io/rerun/issues/963):
        egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));
    });
    match EndpointCategory::categorize_uri(url) {
        EndpointCategory::HttpRrd(url) => Ok(
            re_log_encoding::stream_rrd_from_http::stream_rrd_from_http_to_channel(
                url,
                Some(ui_waker),
            ),
        ),
        EndpointCategory::WebEventListener => {
            // Process an rrd when it's posted via `window.postMessage`
            let (tx, rx) = re_smart_channel::smart_channel(
                re_smart_channel::SmartMessageSource::RrdWebEventCallback,
                re_smart_channel::SmartChannelSource::RrdWebEventListener,
            );
            let url = url.to_owned();
            re_log_encoding::stream_rrd_from_http::stream_rrd_from_event_listener(Arc::new({
                move |msg| {
                    ui_waker();
                    use re_log_encoding::stream_rrd_from_http::HttpMessage;
                    match msg {
                        HttpMessage::LogMsg(msg) => {
                            if tx.send(msg).is_ok() {
                                ControlFlow::Continue(())
                            } else {
                                re_log::info_once!("Failed to send log message to viewer - closing connection to {url}");
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
            Ok(rx)
        }
        EndpointCategory::WebSocket(url) => re_data_source::connect_to_ws_url(&url, Some(ui_waker))
            .with_context(|| format!("Failed to connect to WebSocket server at {url}.")),
    }
}
