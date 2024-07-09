use std::ops::ControlFlow;

use ewebsock::{WsEvent, WsMessage};

// TODO(jleibs): use thiserror
pub type Result<T> = anyhow::Result<T>;

/// Connect viewer to server
pub fn viewer_to_server(
    url: String,
    on_binary_msg: impl Fn(Vec<u8>) -> ControlFlow<()> + Send + 'static,
) -> Result<()> {
    let gigs = 1024 * 1024 * 1024;
    let options = ewebsock::Options {
        // This limits the size of one chunk of rerun log data when running a local websocket client.
        // We set a very high limit, because we should be able to trust the server.
        // See https://github.com/rerun-io/rerun/issues/5268 for more
        max_incoming_frame_size: 2 * gigs,
    };

    ewebsock::ws_receive(
        url.clone(),
        options,
        Box::new(move |event: WsEvent| match event {
            WsEvent::Opened => {
                re_log::info!("Connection to {url} established");
                ControlFlow::Continue(())
            }
            WsEvent::Message(message) => match message {
                WsMessage::Binary(binary) => on_binary_msg(binary),
                WsMessage::Text(text) => {
                    re_log::warn!("Unexpected text message: {text:?}");
                    ControlFlow::Continue(())
                }
                WsMessage::Unknown(text) => {
                    re_log::warn!("Unknown message: {text:?}");
                    ControlFlow::Continue(())
                }
                WsMessage::Ping(_data) => {
                    re_log::warn!("Unexpected PING");
                    ControlFlow::Continue(())
                }
                WsMessage::Pong(_data) => {
                    re_log::warn!("Unexpected PONG");
                    ControlFlow::Continue(())
                }
            },
            WsEvent::Error(error) => {
                re_log::error!("Connection error: {error}");
                ControlFlow::Break(())
            }
            WsEvent::Closed => {
                re_log::info!("Connection to {url} closed.");
                ControlFlow::Break(())
            }
        }),
    )
    .map_err(|err| anyhow::format_err!("ewebsock: {err}"))
}
