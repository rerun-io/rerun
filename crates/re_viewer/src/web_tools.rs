use std::{ops::ControlFlow, sync::Arc};

use anyhow::Context as _;
use wasm_bindgen::JsValue;

use re_log::ResultExt as _;
use re_viewer_context::CommandSender;

/// Web-specific tools used by various parts of the application.

/// Useful in error handlers
#[allow(clippy::needless_pass_by_value)]
pub fn string_from_js_value(s: wasm_bindgen::JsValue) -> String {
    s.as_string().unwrap_or(format!("{s:#?}"))
}

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

pub fn go_back() -> Option<()> {
    let history = web_sys::window()?
        .history()
        .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
        .ok_or_log_error()?;
    history
        .back()
        .map_err(|err| format!("Failed to go back: {}", string_from_js_value(err)))
        .ok_or_log_error()
}

pub fn go_forward() -> Option<()> {
    let history = web_sys::window()?
        .history()
        .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
        .ok_or_log_error()?;
    history
        .forward()
        .map_err(|err| format!("Failed to go forward: {}", string_from_js_value(err)))
        .ok_or_log_error()
}

/// The current percent-encoded URL suffix, e.g. "?foo=bar#baz".
pub fn current_url_suffix() -> Option<String> {
    let location = web_sys::window()?.location();
    let search = location.search().unwrap_or_default();
    let hash = location.hash().unwrap_or_default();
    Some(format!("{search}{hash}"))
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
    let current_relative_url = current_url_suffix().unwrap_or_default();

    if current_relative_url == new_relative_url {
        re_log::debug!("Ignoring navigation to {new_relative_url:?} as we're already there");
    } else {
        re_log::debug!(
            "Existing url is {current_relative_url:?}; navigating to {new_relative_url:?}"
        );

        let history = web_sys::window()?
            .history()
            .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
            .ok_or_log_error()?;
        // Instead of setting state to `null`, try to preserve existing state.
        // This helps with ensuring JS frameworks can perform client-side routing.
        // If we ever need to store anything in `state`, we should rethink how
        // we handle this.
        let existing_state = history.state().unwrap_or(JsValue::NULL);
        history
            .push_state_with_url(&existing_state, "", Some(new_relative_url))
            .map_err(|err| {
                format!(
                    "Failed to push history state: {}",
                    string_from_js_value(err)
                )
            })
            .ok_or_log_error()?;
    }
    Some(())
}

/// Replace the current relative url with an new one.
pub fn replace_history(new_relative_url: &str) -> Option<()> {
    let history = web_sys::window()?
        .history()
        .map_err(|err| format!("Failed to get History API: {}", string_from_js_value(err)))
        .ok_or_log_error()?;
    // NOTE: See `existing_state` in `push_history` above for info on why this is here.
    let existing_state = history.state().unwrap_or(JsValue::NULL);
    history
        .replace_state_with_url(&existing_state, "", Some(new_relative_url))
        .map_err(|err| {
            format!(
                "Failed to push history state: {}",
                string_from_js_value(err)
            )
        })
        .ok_or_log_error()
}

/// Parse the `?query` parst of the url, and translate it into commands to control the application.
pub fn translate_query_into_commands(egui_ctx: &egui::Context, command_sender: &CommandSender) {
    use re_viewer_context::{SystemCommand, SystemCommandSender as _};

    let location = eframe::web::web_location();

    if let Some(app_ids) = location.query_map.get("app_id") {
        if let Some(app_id) = app_ids.last() {
            let app_id = re_log_types::ApplicationId::from(app_id.as_str());
            command_sender.send_system(SystemCommand::ActivateApp(app_id));
        }
    }

    // NOTE: we support passing in multiple urls to multiple different recorording, blueprints, etc
    let urls: Vec<&String> = location
        .query_map
        .get("url")
        .into_iter()
        .flatten()
        .collect();
    if !urls.is_empty() {
        for url in urls {
            if let Some(receiver) = url_to_receiver(egui_ctx.clone(), url).ok_or_log_error() {
                // We may be here because the user clicked Back/Forward in the browser while trying
                // out examples. If we re-download the same file we should clear out the old data first.
                command_sender.send_system(SystemCommand::ClearSourceAndItsStores(
                    receiver.source().clone(),
                ));

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
