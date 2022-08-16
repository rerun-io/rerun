use std::{
    net::SocketAddr,
    sync::mpsc::{Receiver, Sender},
};

use re_log_types::LogMsg;

#[derive(Debug, PartialEq, Eq)]
struct FlushMsg;

enum MsgMsg {
    LogMsg(LogMsg),
    SetAddr(SocketAddr),
    Flush,
}

enum PacketMsg {
    Packet(Vec<u8>),
    SetAddr(SocketAddr),
    Flush,
}

/// Send [`LogMsg`]es to a server.
///
/// The messages are encoded and sent on separate threads
/// so that calling [`BufferedClient::send`] is non-blocking.
pub struct Client {
    msg_tx: Sender<MsgMsg>,
    back_rx: Receiver<FlushMsg>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new(crate::default_server_addr())
    }
}

impl Client {
    pub fn new(addr: SocketAddr) -> Self {
        // TODO(emilk): keep track of how much memory is in each pipe
        // and apply back-pressure to not use too much RAM.
        let (msg_tx, msg_rx) = std::sync::mpsc::channel();
        let (packet_tx, packet_rx) = std::sync::mpsc::channel();
        let (back_tx, back_rx) = std::sync::mpsc::channel();

        std::thread::Builder::new()
            .name("msg_encoder".into())
            .spawn(move || {
                while let Ok(msg_msg) = msg_rx.recv() {
                    let packet_msg = match msg_msg {
                        MsgMsg::LogMsg(log_msg) => {
                            let packet = crate::encode_log_msg(&log_msg);
                            tracing::debug!("Encoded message of size {}", packet.len());
                            PacketMsg::Packet(packet)
                        }
                        MsgMsg::SetAddr(new_addr) => PacketMsg::SetAddr(new_addr),
                        MsgMsg::Flush => PacketMsg::Flush,
                    };

                    packet_tx
                        .send(packet_msg)
                        .expect("tcp_sender thread should live longer");
                }
                tracing::debug!("Shutting down msg encoder thread");
            })
            .expect("Failed to spanw thread");

        std::thread::Builder::new()
            .name("tcp_sender".into())
            .spawn(move || {
                let mut tcp_client = crate::tcp_client::TcpClient::new(addr);
                while let Ok(packet_msg) = packet_rx.recv() {
                    match packet_msg {
                        PacketMsg::Packet(packet) => {
                            tcp_client.send(&packet);
                        }
                        PacketMsg::SetAddr(new_addr) => {
                            tcp_client.set_addr(new_addr);
                        }
                        PacketMsg::Flush => {
                            tcp_client.flush();
                            back_tx
                                .send(FlushMsg)
                                .expect("Main thread should still be alive");
                        }
                    }
                }

                tracing::debug!("Shutting down TCP sender thread");
            })
            .expect("Failed to spanw thread");

        Self { msg_tx, back_rx }
    }

    pub fn set_addr(&mut self, addr: SocketAddr) {
        self.msg_tx
            .send(MsgMsg::SetAddr(addr))
            .expect("msg_encoder should not shut down until we tell it to");
    }

    pub fn send(&mut self, log_msg: LogMsg) {
        tracing::trace!("Sending message…");
        self.msg_tx
            .send(MsgMsg::LogMsg(log_msg))
            .expect("msg_encoder should not shut down until we tell it to");
    }

    /// Stall untill all messages so far has been sent.
    pub fn flush(&mut self) {
        tracing::debug!("Flushing message queue…");

        self.msg_tx
            .send(MsgMsg::Flush)
            .expect("msg_encoder should not shut down until we tell it to");

        match self.back_rx.recv() {
            Ok(FlushMsg) => {
                tracing::debug!("Flush complete.");
            }
            Err(_) => {
                // This should really never happen
                tracing::error!("Failed to flush pipeline");
            }
        }
    }
}

impl Drop for Client {
    /// Wait until everything has been sent.
    fn drop(&mut self) {
        self.flush();
        tracing::debug!("Sender has shut down.");
    }
}
