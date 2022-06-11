use ewebsock::*;
use log_types::DataMsg;
use std::ops::ControlFlow;

use crate::{decode_log_msg, Result};

/// Represents a connection to the server.
/// Disconnects on drop.
#[must_use]
pub struct Connection(WsSender);

impl Connection {
    /// Connect viewer to ser
    pub fn viewer_to_server(
        url: String,
        on_log_msg: impl Fn(DataMsg) -> ControlFlow<()> + Send + 'static,
    ) -> Result<Self> {
        let sender = ewebsock::ws_connect(
            url,
            Box::new(move |event: WsEvent| match event {
                WsEvent::Opened => {
                    tracing::info!("Connection established");
                    ControlFlow::Continue(())
                }
                WsEvent::Message(message) => match message {
                    WsMessage::Binary(binary) => match decode_log_msg(&binary) {
                        Ok(data_msg) => on_log_msg(data_msg),
                        Err(err) => {
                            tracing::error!("Failed to parse message: {:?}", err);
                            ControlFlow::Break(())
                        }
                    },
                    WsMessage::Text(text) => {
                        tracing::warn!("Unexpected text message: {:?}", text);
                        ControlFlow::Continue(())
                    }
                    WsMessage::Unknown(text) => {
                        tracing::warn!("Unknown message: {:?}", text);
                        ControlFlow::Continue(())
                    }
                    WsMessage::Ping(_data) => {
                        tracing::warn!("Unexpected PING");
                        ControlFlow::Continue(())
                    }
                    WsMessage::Pong(_data) => {
                        tracing::warn!("Unexpected PONG");
                        ControlFlow::Continue(())
                    }
                },
                WsEvent::Error(error) => {
                    tracing::error!("Connection error: {}", error);
                    ControlFlow::Break(())
                }
                WsEvent::Closed => {
                    tracing::info!("Connection to server closed.");
                    ControlFlow::Break(())
                }
            }),
        )
        .map_err(|err| anyhow::format_err!("ewebsock: {}", err))?;

        Ok(Self(sender))
    }
}
