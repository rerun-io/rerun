//! The server is a pub-sub architecture.
//!
//! Each incoming log message is stored, and sent to any connected client.
//! Each connecting client is first sent the history of stored log messages.
//!
//! In the future thing will be changed to a protocol where the clients can query
//! for specific data based on e.g. time.

use std::{net::SocketAddr, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Error};

use re_log_types::LogMsg;
use re_smart_channel::Receiver;

use crate::{server_url, RerunServerError, RerunServerPort};

/// Websocket host for relaying [`LogMsg`]s to a web viewer.
pub struct RerunServer {
    listener: TcpListener,
    local_addr: std::net::SocketAddr,
}

impl RerunServer {
    /// Create new [`RerunServer`] to relay [`LogMsg`]s to a websocket.
    /// The websocket will be available at `port`.
    ///
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    /// A port of 0 will let the OS choose a free port.
    pub async fn new(bind_ip: String, port: RerunServerPort) -> Result<Self, RerunServerError> {
        let bind_addr = format!("{bind_ip}:{port}");

        let listener = TcpListener::bind(&bind_addr)
            .await
            .map_err(|err| RerunServerError::BindFailed(port, err))?;

        let slf = Self {
            local_addr: listener.local_addr()?,
            listener,
        };

        re_log::info!(
            "Listening for websocket traffic on {}. Connect with a Rerun Web Viewer.",
            slf.server_url()
        );

        Ok(slf)
    }

    /// Accept new connections
    pub async fn listen(self, rx: Receiver<LogMsg>) -> Result<(), RerunServerError> {
        let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        self.listen_with_graceful_shutdown(rx, shutdown_rx).await
    }

    /// Accept new connections until we get a message on `shutdown_rx`
    pub async fn listen_with_graceful_shutdown(
        self,
        rx: Receiver<LogMsg>,
        mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), RerunServerError> {
        let history = Arc::new(Mutex::new(Vec::new()));

        let log_stream = to_broadcast_stream(rx, history.clone());

        loop {
            let (tcp_stream, _) = tokio::select! {
                res = self.listener.accept() => res?,
                _ = shutdown_rx.recv() => {
                    return Ok(());
                }
            };

            let peer = tcp_stream.peer_addr()?;
            tokio::spawn(accept_connection(
                log_stream.clone(),
                peer,
                tcp_stream,
                history.clone(),
            ));
        }
    }

    /// Contains the `ws://` or `wss://` prefix.
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }
}

/// Sync handle for the [`RerunServer`]
///
/// When dropped, the server will be shut down.
pub struct RerunServerHandle {
    local_addr: std::net::SocketAddr,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl Drop for RerunServerHandle {
    fn drop(&mut self) {
        re_log::info!("Shutting down Rerun server on {}", self.server_url());
        self.shutdown_tx.send(()).ok();
    }
}

impl RerunServerHandle {
    /// Create new [`RerunServer`] to relay [`LogMsg`]s to a websocket.
    /// Returns a [`RerunServerHandle`] that will shutdown the server when dropped.
    ///
    /// A `bind_ip` of `"0.0.0.0"` is a good default.
    /// A port of 0 will let the OS choose a free port.
    ///
    /// The caller needs to ensure that there is a `tokio` runtime running.
    pub fn new(
        rerun_rx: Receiver<LogMsg>,
        bind_ip: String,
        requested_port: RerunServerPort,
    ) -> Result<Self, RerunServerError> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

        let rt = tokio::runtime::Handle::current();

        let ws_server = rt.block_on(tokio::spawn(async move {
            RerunServer::new(bind_ip, requested_port).await
        }))??;

        let local_addr = ws_server.local_addr;

        tokio::spawn(async move {
            ws_server
                .listen_with_graceful_shutdown(rerun_rx, shutdown_rx)
                .await
        });

        Ok(Self {
            local_addr,
            shutdown_tx,
        })
    }

    /// Contains the `ws://` or `wss://` prefix.
    pub fn server_url(&self) -> String {
        server_url(&self.local_addr)
    }
}

fn to_broadcast_stream(
    log_rx: Receiver<LogMsg>,
    history: Arc<Mutex<Vec<Arc<[u8]>>>>,
) -> tokio::sync::broadcast::Sender<Arc<[u8]>> {
    let (tx, _) = tokio::sync::broadcast::channel(1024 * 1024);
    let tx1 = tx.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(msg) = log_rx.recv() {
            match msg.payload {
                re_smart_channel::SmartMessagePayload::Msg(data) => {
                    let bytes = crate::encode_log_msg(&data);
                    let bytes: Arc<[u8]> = bytes.into();
                    history.lock().push(bytes.clone());
                    if let Err(tokio::sync::broadcast::error::SendError(_bytes)) = tx1.send(bytes) {
                        // no receivers currently - that's fine!
                    }
                }
                re_smart_channel::SmartMessagePayload::Quit(err) => {
                    if let Some(err) = err {
                        re_log::warn!(%msg.source, err, "sender has left unexpectedly");
                    } else {
                        re_log::debug!(%msg.source, "sender has left");
                    }
                }
            }
        }
    });
    tx
}

async fn accept_connection(
    log_stream: tokio::sync::broadcast::Sender<Arc<[u8]>>,
    _peer: SocketAddr,
    tcp_stream: TcpStream,
    history: Arc<Mutex<Vec<Arc<[u8]>>>>,
) {
    // let span = re_log::span!(
    //     re_log::Level::INFO,
    //     "Connection",
    //     peer = _peer.to_string().as_str()
    // );
    // let _enter = span.enter();

    re_log::debug!("New WebSocket connection");

    if let Err(err) = handle_connection(log_stream, tcp_stream, history).await {
        match err {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => re_log::error!("Error processing connection: {err}"),
        }
    }
}

async fn handle_connection(
    log_stream: tokio::sync::broadcast::Sender<Arc<[u8]>>,
    tcp_stream: TcpStream,
    history: Arc<Mutex<Vec<Arc<[u8]>>>>,
) -> tungstenite::Result<()> {
    let ws_stream = accept_async(tcp_stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Re-sending packet history - this is not water tight, but better than nothing.
    // TODO(emilk): water-proof resending of history + streaming of new stuff, without anything missed.
    let history = history.lock().to_vec();
    for packet in history {
        ws_sender
            .send(tungstenite::Message::Binary(packet.to_vec()))
            .await?;
    }

    let mut log_rx = log_stream.subscribe();

    loop {
        tokio::select! {
            ws_msg = ws_receiver.next() => {
                match ws_msg {
                    Some(Ok(msg)) => {
                        re_log::debug!("Received message: {:?}", msg);
                    }
                    Some(Err(err)) => {
                        re_log::warn!("Error message: {err}");
                        break;
                    }
                    None => {
                        break;
                    }
                }
            }
            data_msg = log_rx.recv() => {
                let data_msg = data_msg.unwrap();

                ws_sender.send(tungstenite::Message::Binary(data_msg.to_vec())).await?;
            }
        }
    }

    Ok(())
}
