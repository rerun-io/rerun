use std::{
    net::SocketAddr,
    sync::mpsc::{Receiver, Sender},
};

use re_log_types::LogMsg;

#[derive(Debug, PartialEq, Eq)]
struct ShutdownMsg;

/// Send [`LogMsg`]es to a server.
///
/// The messages are encoded and sent on separate threads
/// so that calling [`BufferedClient::send`] is non-blocking.
pub struct Client {
    msg_tx: Sender<LogMsg>,
    shutdown_rx: Receiver<ShutdownMsg>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new(crate::default_server_addr())
    }
}

impl Client {
    pub fn new(addr: SocketAddr) -> Self {
        let (msg_tx, msg_rx) = std::sync::mpsc::channel();
        let (packet_tx, packet_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

        std::thread::Builder::new()
            .name("msg_encoder".into())
            .spawn(move || {
                while let Ok(log_msg) = msg_rx.recv() {
                    let packet = crate::encode_log_msg(&log_msg);
                    tracing::trace!("Encoded message of size {}", packet.len());
                    packet_tx
                        .send(packet)
                        .expect("tcp_sender thread should live longer");
                }
                tracing::debug!("Shutting down msg encoder thread");
            })
            .expect("Failed to spanw thread");

        std::thread::Builder::new()
            .name("tcp_sender".into())
            .spawn(move || {
                let mut tcp_client = crate::tcp_client::TcpClient::new(addr);
                while let Ok(packet) = packet_rx.recv() {
                    tcp_client.send(&packet);
                }
                tcp_client.flush();

                tracing::debug!("Shutting down TCP sender thread");

                shutdown_tx
                    .send(ShutdownMsg)
                    .expect("Main thread should still be alive");
            })
            .expect("Failed to spanw thread");

        Self {
            msg_tx,
            shutdown_rx,
        }
    }

    pub fn send(&mut self, log_msg: LogMsg) {
        tracing::trace!("Sending message…");
        self.msg_tx
            .send(log_msg)
            .expect("msg_encoder should not shut down until we tell it to");
    }
}

impl Drop for Client {
    /// Wait until everything has been sent.
    fn drop(&mut self) {
        {
            // An ugly way of dropping self.msg_tx:
            let (dummy_tx, _dummy_rx) = std::sync::mpsc::channel();
            drop(std::mem::replace(&mut self.msg_tx, dummy_tx));
        }

        tracing::debug!("Waiting for sender to shut down…");
        let shutdown_result = self.shutdown_rx.recv();
        debug_assert_eq!(shutdown_result, Ok(ShutdownMsg));
        tracing::debug!("Sender has shut down.");
    }
}
