use std::net::SocketAddr;

use crossbeam::channel::{select, Receiver, Sender};

use re_log_types::{LogMsg, MsgId};

#[derive(Debug, PartialEq, Eq)]
struct FlushedMsg;

/// Sent to prematurely quit (before flushing).
#[derive(Debug, PartialEq, Eq)]
struct QuitMsg;

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
/// so that calling [`Client::send`] is non-blocking.
pub struct Client {
    msg_tx: Sender<MsgMsg>,
    flushed_rx: Receiver<FlushedMsg>,
    encode_quit_tx: Sender<QuitMsg>,
    send_quit_tx: Sender<QuitMsg>,
    drop_quit_tx: Sender<QuitMsg>,
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
        let (msg_tx, msg_rx) = crossbeam::channel::unbounded();
        let (msg_drop_tx, msg_drop_rx) = crossbeam::channel::unbounded();
        let (packet_tx, packet_rx) = crossbeam::channel::unbounded();
        let (flushed_tx, flushed_rx) = crossbeam::channel::unbounded();
        let (encode_quit_tx, encode_quit_rx) = crossbeam::channel::unbounded();
        let (send_quit_tx, send_quit_rx) = crossbeam::channel::unbounded();
        let (drop_quit_tx, drop_quit_rx) = crossbeam::channel::unbounded();

        std::thread::Builder::new()
            .name("msg_encoder".into())
            .spawn(move || {
                msg_encode(&msg_rx, &msg_drop_tx, &encode_quit_rx, &packet_tx);
                re_log::debug!("Shutting down msg encoder thread");
            })
            .expect("Failed to spawn thread");

        std::thread::Builder::new()
            .name("msg_dropper".into())
            .spawn(move || {
                msg_drop(&msg_drop_rx, &drop_quit_rx);
                re_log::debug!("Shutting down msg dropper thread");
            })
            .expect("Failed to spawn thread");

        std::thread::Builder::new()
            .name("tcp_sender".into())
            .spawn(move || {
                tcp_sender(addr, &packet_rx, &send_quit_rx, &flushed_tx);
                re_log::debug!("Shutting down TCP sender thread");
            })
            .expect("Failed to spawn thread");

        Self {
            msg_tx,
            flushed_rx,
            encode_quit_tx,
            send_quit_tx,
            drop_quit_tx,
        }
    }

    pub fn set_addr(&mut self, addr: SocketAddr) {
        self.send_msg_msg(MsgMsg::SetAddr(addr));
    }

    pub fn send(&mut self, log_msg: LogMsg) {
        self.send_msg_msg(MsgMsg::LogMsg(log_msg));
    }

    /// Stall untill all messages so far has been sent.
    pub fn flush(&mut self) {
        re_log::debug!("Flushing message queue…");
        self.send_msg_msg(MsgMsg::Flush);

        match self.flushed_rx.recv() {
            Ok(FlushedMsg) => {
                re_log::debug!("Flush complete.");
            }
            Err(_) => {
                // This can happen on Ctrl-C
                re_log::warn!("Failed to flush pipeline - not all messages were sent (Ctrl-C).");
            }
        }
    }

    fn send_msg_msg(&mut self, msg: MsgMsg) {
        // ignoring errors, because Ctrl-C can shut down the receiving end.
        self.msg_tx.send(msg).ok();
    }
}

impl Drop for Client {
    /// Wait until everything has been sent.
    fn drop(&mut self) {
        re_log::debug!("Shutting down the client connection…");
        self.send(LogMsg::Goodbye(MsgId::random()));
        self.flush();
        self.encode_quit_tx.send(QuitMsg).ok();
        self.send_quit_tx.send(QuitMsg).ok();
        self.drop_quit_tx.send(QuitMsg).ok();
        re_log::debug!("Sender has shut down.");
    }
}

// We drop messages in a separate thread because the PyO3 + Arrow memory model
// means in some cases these messages actually store pointers back to
// python-managed memory. We don't want to block our send-thread waiting for the
// GIL.
fn msg_drop(msg_drop_rx: &Receiver<MsgMsg>, quit_rx: &Receiver<QuitMsg>) {
    loop {
        select! {
            recv(msg_drop_rx) -> msg_msg => {
                if let Ok(_) = msg_msg {
                } else {
                    return; // channel has closed
                }
            }
            recv(quit_rx) -> _quit_msg => {
                return;
            }
        }
    }
}

fn msg_encode(
    msg_rx: &Receiver<MsgMsg>,
    msg_drop_tx: &Sender<MsgMsg>,
    quit_rx: &Receiver<QuitMsg>,
    packet_tx: &Sender<PacketMsg>,
) {
    loop {
        select! {
            recv(msg_rx) -> msg_msg => {
                if let Ok(msg_msg) = msg_msg {
                    let packet_msg = match &msg_msg {
                        MsgMsg::LogMsg(log_msg) => {
                            let packet = crate::encode_log_msg(log_msg);
                            re_log::trace!("Encoded message of size {}", packet.len());
                            PacketMsg::Packet(packet)
                        }
                        MsgMsg::SetAddr(new_addr) => PacketMsg::SetAddr(new_addr.clone()),
                        MsgMsg::Flush => PacketMsg::Flush,
                    };

                    packet_tx
                        .send(packet_msg)
                        .expect("tcp_sender thread should live longer");
                    msg_drop_tx.send(msg_msg).expect("Main thread should still be alive");
                } else {
                    return; // channel has closed
                }
            }
            recv(quit_rx) -> _quit_msg => {
                return;
            }
        }
    }
}

fn tcp_sender(
    addr: SocketAddr,
    packet_rx: &Receiver<PacketMsg>,
    quit_rx: &Receiver<QuitMsg>,
    flushed_tx: &Sender<FlushedMsg>,
) {
    let mut tcp_client = crate::tcp_client::TcpClient::new(addr);

    loop {
        select! {
            recv(packet_rx) -> packet_msg => {
                if let Ok(packet_msg) = packet_msg {
                    match packet_msg {
                        PacketMsg::Packet(packet) => {
                            if send_until_success(&mut tcp_client, &packet, quit_rx) == Some(QuitMsg) {
                                return;
                            }
                        }
                        PacketMsg::SetAddr(new_addr) => {
                            tcp_client.set_addr(new_addr);
                        }
                        PacketMsg::Flush => {
                            tcp_client.flush();
                            flushed_tx
                                .send(FlushedMsg)
                                .expect("Main thread should still be alive");
                        }
                    }
                } else {
                    return; // channel has closed
                }
            }
            recv(quit_rx) -> _quit_msg => {
                return;
            }
        }
    }
}

fn send_until_success(
    tcp_client: &mut crate::tcp_client::TcpClient,
    packet: &[u8],
    quit_rx: &Receiver<QuitMsg>,
) -> Option<QuitMsg> {
    if let Err(err) = tcp_client.send(packet) {
        re_log::warn!("Failed to send message: {err}");

        let mut sleep_ms = 100;

        loop {
            select! {
                recv(quit_rx) -> _quit_msg => {
                    return Some(QuitMsg);
                }
                default(std::time::Duration::from_millis(sleep_ms)) => {
                    if let Err(new_err) = tcp_client.send(packet) {
                        if new_err.to_string() != err.to_string() {
                            re_log::warn!("Failed to send message: {err}");
                        }
                        sleep_ms = (sleep_ms * 2).min(3000);
                    } else {
                        return None;
                    }
                }
            }
        }
    } else {
        None
    }
}
