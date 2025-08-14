use re_data_source::LogDataSource;
use re_smart_channel::SmartChannelSource;
use re_viewer_context::{
    CommandSender, DisplayMode, Item, StoreHub, SystemCommand, SystemCommandSender as _,
};

/// A URL that points to a selection (typically an entity) within the currently active recording.
pub const INTRA_RECORDING_URL_SCHEME: &str = "recording://";

/// An eventListener for rrd posted from containing html
pub const WEB_EVENT_LISTENER_SCHEME: &str = "web_event:";

/// Tries to open a content URL or file inside the viewer.
///
/// This is for handling opening arbitrary URLs inside the viewer
/// (as opposed to opening them in a new tab) for both native and web.
/// Supported are:
/// * any URL or file path that can be interpreted as a [`DataSource`]
/// * intra-recording links (typically links to an entity)
/// * web event listeners
///
/// This is the highest level way of opening arbitrary URLs inside the viewer.
/// The only higher level way of opening URLs is `ui.ctx().open_url(...)` which will
/// open the URL in a browser if it's not a content URL that we can open inside the viewer.
///
/// Returns `Ok(())` if the URL schema was recognized, `Err(())` if the URL was not a valid content URL.
pub fn try_open_url_or_file_in_viewer(
    _egui_ctx: &egui::Context,
    url: &str,
    follow_if_http: bool,
    select_redap_source_when_loaded: bool,
    command_sender: &CommandSender,
) -> Result<(), ()> {
    re_log::debug!("Opening URL: {url:?}");

    if let Ok(uri) = url.parse::<re_uri::CatalogUri>() {
        command_sender.send_system(SystemCommand::AddRedapServer(uri.origin.clone()));
        command_sender.send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(
            uri.origin,
        )));
    } else if let Ok(uri) = url.parse::<re_uri::EntryUri>() {
        command_sender.send_system(SystemCommand::AddRedapServer(uri.origin));
        command_sender.send_system(SystemCommand::SetSelection(Item::RedapEntry(uri.entry_id)));
    } else if let Some(mut data_source) =
        LogDataSource::from_uri(re_log_types::FileSource::Uri, url)
    {
        if let LogDataSource::RedapDataset {
            select_when_loaded, ..
        } = &mut data_source
        {
            // `select_when_loaded` is not encoded in the url itself. As of writing, `DataSource::from_uri` will just always set `select_when_loaded` to `true`.
            // We overwrite this with the passed in value.
            *select_when_loaded = select_redap_source_when_loaded;
        } else if let LogDataSource::RrdHttpUrl { follow, .. } = &mut data_source {
            // `follow` is not encoded in the url itself. As of writing, `DataSource::from_uri` will just always set `follow` to `false`.
            // We overwrite this with the passed in value.
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

    command_sender.send_system(SystemCommand::AddReceiver(rx));
}

/// Tries to create a content URL for the current display mode that can be shared.
///
/// Note that the returned URL does not contain the "web viewer hosting" part.
/// I.e. you can import this URL in a Rerun viewer but you can't necessarily put it in your browser address bar.
///
/// Returns `None` if the current state can't be shared with a url.
// TODO(andreas): We'll need this for our "share link" editor again, but with lots more knobs. Probably need to split this.
//                We can likely have a more structured one for redap urls and have this higher level function use that (share editor is only planned for redap urls)
// TODO(#10866): Should have anchors for selection etc. when supported.
#[allow(unused)] // only used in web viewer as of writing
pub fn display_mode_to_content_url(
    store_hub: &StoreHub,
    display_mode: &DisplayMode,
) -> Option<String> {
    re_log::debug!("Updating navigation bar");

    match display_mode {
        DisplayMode::Settings => {
            // Not much point in updating address for the settings screen.
            None
        }
        DisplayMode::LocalRecordings => {
            // Local recordings includes those downloaded from rrd urls
            // (as of writing this includes the sample recordings!)
            // If it's one of those we want to update the address bar accordingly.

            let active_recording = store_hub.active_recording()?;
            let data_source = active_recording.data_source.as_ref()?;

            match data_source {
                SmartChannelSource::RrdHttpStream { url, follow: _ } => Some(url.clone()),

                SmartChannelSource::File(_path_buf) => {
                    // Can't share links to local files.
                    None
                }

                SmartChannelSource::RrdWebEventListener
                | SmartChannelSource::JsChannel { .. }
                | SmartChannelSource::Sdk
                | SmartChannelSource::Stdin => {
                    // Can't share links to live streams / local events.
                    None
                }

                SmartChannelSource::RedapGrpcStream {
                    uri,
                    select_when_loaded: _,
                } => Some(uri.to_string()),

                SmartChannelSource::MessageProxy(proxy_uri) => Some(proxy_uri.to_string()),
            }
        }

        DisplayMode::LocalTable(_table_id) => {
            // We can't share links to local tables, so can't update the url.
            None
        }

        DisplayMode::RedapEntry(_entry_id) => {
            // TODO(#10866): Implement this.
            None
        }

        DisplayMode::RedapServer(origin) => {
            // `as_url` on the origin gives us an http link.
            // We want a rerun link here instead.
            Some(re_uri::RedapUri::Catalog(re_uri::CatalogUri::new(origin.clone())).to_string())
        }

        DisplayMode::ChunkStoreBrowser => {
            // As of writing the store browser is more of a debugging feature.
            None
        }
    }
}
