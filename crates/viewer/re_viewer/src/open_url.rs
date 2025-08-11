use re_data_source::DataSource;
use re_viewer_context::{CommandSender, Item, SystemCommand, SystemCommandSender as _};

/// A URL that points to a a selection (typically an entity) within the currently active recording.
pub const INTRA_RECORDING_URL_SCHEME: &str = "recording://";

/// An eventListener for rrd posted from containing html
pub const WEB_EVENT_LISTENER_SCHEME: &str = "web_event:";

/// Tries to open a content URL inside the viewer.
///
/// This is for handling opening arbitrary URLs inside the viewer
/// (as opposed to opening them in a new tab) for both native and web.
/// Supported are:
/// * any URL that can be interpreted as a [`DataSource`]
/// * intra-recording links (typically links to an entity)
/// * web event listeners
///
/// This is the highest level way of opening arbitrary URLs inside the viewer.
/// The only higher level way of opening URLs is `ui.ctx().open_url(...)` which will
/// open the URL in a browser if it's not a content URL that we can open inside the viewer.
///
/// Returns `Ok(())` if the URL schema was recognized, `Err(())` if the URL was not a valid content URL.
pub fn try_open_url_in_viewer(
    _egui_ctx: &egui::Context,
    url: &str,
    follow_if_http: bool,
    select_redap_source_when_loaded: bool,
    command_sender: &CommandSender,
) -> Result<(), ()> {
    re_log::debug!("Opening URL: {url:?}");

    if let Some(mut data_source) = DataSource::from_uri(re_log_types::FileSource::Uri, url) {
        if let DataSource::RerunGrpcStream {
            select_when_loaded, ..
        } = &mut data_source
        {
            *select_when_loaded = select_redap_source_when_loaded;
        } else if let DataSource::RrdHttpUrl { follow, .. } = &mut data_source {
            *follow = follow_if_http;
        }

        command_sender.send_system(SystemCommand::LoadDataSource(data_source));
    } else if let Some(selection) = url.strip_prefix(INTRA_RECORDING_URL_SCHEME) {
        match selection.parse::<Item>() {
            Ok(item) => {
                command_sender.send_system(SystemCommand::SetSelection(item));
            }
            Err(err) => {
                re_log::warn!("Failed to parse selection path {selection:?}: {err}");
            }
        }
    } else if let Some(url) = url.strip_prefix(WEB_EVENT_LISTENER_SCHEME) {
        handle_web_event_listener(_egui_ctx, url, command_sender);
    } else {
        return Err(());
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_web_event_listener(
    _egui_ctx: &egui::Context,
    url: &str,
    _command_sender: &CommandSender,
) {
    re_log::warn!(
        "Can't open {url}: {WEB_EVENT_LISTENER_SCHEME:?} urls are only available on the web viewer."
    );
}

#[cfg(target_arch = "wasm32")]
fn handle_web_event_listener(egui_ctx: &egui::Context, url: &str, command_sender: &CommandSender) {
    use re_log::ResultExt as _;
    use re_log_encoding::stream_rrd_from_http::HttpMessage;
    use std::{ops::ControlFlow, sync::Arc};

    // Process an rrd when it's posted via `window.postMessage`
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RrdWebEventCallback,
        re_smart_channel::SmartChannelSource::RrdWebEventListener,
    );
    let egui_ctx = egui_ctx.clone();
    let url = url.to_owned();
    re_log_encoding::stream_rrd_from_http::stream_rrd_from_event_listener(Arc::new({
        move |msg| {
            egui_ctx.request_repaint_after(std::time::Duration::from_millis(10));

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

    // TODO: make this work via the `LoadDataSource` command instead.
    command_sender.send_system(SystemCommand::AddReceiver(rx));
}
