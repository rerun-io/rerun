//! The server is a pub-sub architecture.
//!
//! Each incoming log message is stored, and sent to any connected client.
//! Each connecting client is first sent the history of stored log messages.
//!
//! In the future thing will be changed to a protocol where the clients can query
//! for specific data based on e.g. time.

use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Error};

use re_log_types::LogMsg;

// ----------------------------------------------------------------------------

pub struct Server {
    listener: TcpListener,
}

impl Server {
    /// Start a pub-sub server listening on the given port
    pub async fn new(port: u16) -> anyhow::Result<Self> {
        use anyhow::Context as _;

        let bind_addr = format!("127.0.0.1:{}", port);

        let listener = TcpListener::bind(&bind_addr)
            .await
            .with_context(|| format!("Can't listen on {:?}", bind_addr))?;

        eprintln!("Listening for websocket traffic on: {}", bind_addr);

        Ok(Self { listener })
    }

    /// Accept new connections forever
    pub async fn listen(self, rx: std::sync::mpsc::Receiver<LogMsg>) -> anyhow::Result<()> {
        use anyhow::Context as _;

        let history = Arc::new(Mutex::new(Vec::new()));

        let log_stream = to_broadcast_stream(rx, history.clone());

        while let Ok((tcp_stream, _)) = self.listener.accept().await {
            let peer = tcp_stream
                .peer_addr()
                .context("connected streams should have a peer address")?;
            tokio::spawn(accept_connection(
                log_stream.clone(),
                peer,
                tcp_stream,
                history.clone(),
            ));
        }

        Ok(())
    }
}

fn to_broadcast_stream(
    log_rx: std::sync::mpsc::Receiver<LogMsg>,
    history: Arc<Mutex<Vec<Arc<[u8]>>>>,
) -> tokio::sync::broadcast::Sender<Arc<[u8]>> {
    let (tx, _) = tokio::sync::broadcast::channel(1024);
    let tx1 = tx.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(log_msg) = log_rx.recv() {
            let bytes = crate::encode_log_msg(&log_msg);
            let bytes: Arc<[u8]> = bytes.into();
            history.lock().push(bytes.clone());

            if let Err(tokio::sync::broadcast::error::SendError(_bytes)) = tx1.send(bytes) {
                // no receivers currently - that's fine!
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

    re_log::info!("New WebSocket connection");

    if let Err(e) = handle_connection(log_stream, tcp_stream, history).await {
        match e {
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
    let ws_stream = accept_async(tcp_stream).await.expect("Failed to accept");
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut interval = tokio::time::interval(Duration::from_millis(1000));

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
                        re_log::warn!("Error message: {err:?}");
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
            _ = interval.tick() => {
                // ws_sender.send(Message::Text("tick".to_owned())).await?;
            }
        }
    }

    Ok(())
}
