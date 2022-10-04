use ewebsock::*;
use re_log_types::LogMsg;
use std::ops::ControlFlow;

use crate::{decode_log_msg, Result};

/// Represents a connection to the server.
/// Disconnects on drop.
#[must_use]
pub struct Connection(WsSender);

impl Connection {
    /// Connect viewer to server
    pub fn viewer_to_server(
        url: String,
        on_log_msg: impl Fn(LogMsg) -> ControlFlow<()> + Send + 'static,
    ) -> Result<Self> {
        re_log::info!("Connecting to {url:?}…");
        let sender = ewebsock::ws_connect(
            url,
            Box::new(move |event: WsEvent| match event {
                WsEvent::Opened => {
                    re_log::info!("Connection established");
                    ControlFlow::Continue(())
                }
                WsEvent::Message(message) => match message {
                    WsMessage::Binary(binary) => match decode_log_msg(&binary) {
                        Ok(log_msg) => on_log_msg(log_msg),
                        Err(err) => {
                            re_log::error!("Failed to parse message: {}", re_error::format(&err));
                            ControlFlow::Break(())
                        }
                    },
                    WsMessage::Text(text) => {
                        re_log::warn!("Unexpected text message: {:?}", text);
                        ControlFlow::Continue(())
                    }
                    WsMessage::Unknown(text) => {
                        re_log::warn!("Unknown message: {:?}", text);
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
                    re_log::error!("Connection error: {}", error);
                    ControlFlow::Break(())
                }
                WsEvent::Closed => {
                    re_log::info!("Connection to server closed.");
                    ControlFlow::Break(())
                }
            }),
        )
        .map_err(|err| anyhow::format_err!("ewebsock: {err}"))?;

        Ok(Self(sender))
    }
}
