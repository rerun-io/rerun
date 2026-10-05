use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use rerun::external::{re_error, re_log};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite;

use super::protocol::Message;

const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// A WebSocket client that sends [`Message`]s from the Rerun viewer to the app's control server.
///
/// Messages sent through a [`ControlViewerHandle`] while disconnected are queued,
/// and delivered once [`Self::run`] reconnects.
///
/// # Examples
/// ```
/// # use custom_callback::comms::viewer::ControlViewer;
/// # use custom_callback::comms::protocol::Message;
/// # async fn example() {
/// let viewer = ControlViewer::new("ws://127.0.0.1:9091/ws");
/// let handle = viewer.handle();
///
/// // Spawn the connection handling task
/// tokio::spawn(viewer.run());
///
/// // Send messages through the handle
/// handle.send(Message::Point3d {
///     path: "path".to_owned(),
///     position: (1.0, 2.0, 3.0),
///     radius: 1.0,
/// }).unwrap();
/// # }
/// ```
#[derive(Debug)]
pub struct ControlViewer {
    url: String,
    tx: UnboundedSender<Message>,
    rx: UnboundedReceiver<Message>,
}

/// A [`Clone`] handle to the send queue of a [`ControlViewer`].
#[derive(Clone)]
pub struct ControlViewerHandle {
    tx: UnboundedSender<Message>,
}

impl ControlViewerHandle {
    pub fn send(&self, msg: Message) -> Result<(), tokio::sync::mpsc::error::SendError<Message>> {
        self.tx.send(msg)
    }
}

impl ControlViewer {
    pub fn new(url: impl Into<String>) -> Self {
        #[expect(clippy::disallowed_methods)] // an unbounded_channel is ok for this example
        let (tx, rx) = unbounded_channel();
        Self {
            url: url.into(),
            tx,
            rx,
        }
    }

    pub fn handle(&self) -> ControlViewerHandle {
        ControlViewerHandle {
            tx: self.tx.clone(),
        }
    }

    /// Connects to the server and sends queued messages, reconnecting whenever the connection drops.
    ///
    /// Returns once every [`ControlViewerHandle`] is dropped.
    pub async fn run(mut self) {
        // Keeping our own sender would stop `recv` from ever returning `None`.
        drop(self.tx);

        loop {
            match tokio_tungstenite::connect_async(&self.url).await {
                Ok((socket, _)) => {
                    re_log::info!("Connected to {}", self.url);
                    if Self::send_queued(socket, &mut self.rx).await.is_break() {
                        return;
                    }
                    re_log::info!("Connection lost. Attempting to reconnect…");
                }
                Err(err) => {
                    re_log::error!(
                        "Failed to connect: {}\nURL: {}",
                        re_error::format_ref(&err),
                        self.url
                    );
                }
            }

            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    }

    /// Sends messages until the connection drops (`Continue`) or every handle is gone (`Break`).
    async fn send_queued(
        mut socket: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        rx: &mut UnboundedReceiver<Message>,
    ) -> std::ops::ControlFlow<()> {
        loop {
            tokio::select! {
                message = rx.recv() => {
                    let Some(message) = message else {
                        socket.close(None).await.ok();
                        return std::ops::ControlFlow::Break(());
                    };
                    let json = match serde_json::to_string(&message) {
                        Ok(json) => json,
                        Err(err) => {
                            re_log::error!("Failed to encode message: {}", re_error::format_ref(&err));
                            continue;
                        }
                    };
                    if let Err(err) = socket.send(tungstenite::Message::text(json)).await {
                        re_log::error!("Failed to send message: {}", re_error::format_ref(&err));
                        return std::ops::ControlFlow::Continue(());
                    }
                }

                // The server never sends anything; reading only notices when it goes away.
                frame = socket.next() => match frame {
                    None | Some(Err(_) | Ok(tungstenite::Message::Close(_))) => {
                        return std::ops::ControlFlow::Continue(());
                    }
                    Some(Ok(_)) => {}
                },
            }
        }
    }
}
