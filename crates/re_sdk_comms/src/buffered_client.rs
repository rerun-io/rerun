use std::{fmt, net::SocketAddr, thread::JoinHandle};

use crossbeam::channel::{select, Receiver, Sender};

use re_log_types::LogMsg;

#[derive(Debug, PartialEq, Eq)]
struct FlushedMsg;

/// Sent to prematurely quit (before flushing).
#[derive(Debug, PartialEq, Eq)]
struct QuitMsg;

/// Sent to prematurely quit (before flushing).
#[derive(Debug, PartialEq, Eq)]
enum InterruptMsg {
    /// Switch to a mode where we drop messages if disconnected.
    ///
    /// Sending this before a flush ensures we won't get stuck trying to send
    /// messages to a closed endpoint, but we will still send all messages to an open endpoint.
    DropIfDisconnected,

    /// Quite immediately, dropping any unsent message.
    Quit,
}

enum MsgMsg {
    LogMsg(LogMsg),
    Flush,
}

enum PacketMsg {
    Packet(Vec<u8>),
    Flush,
}

/// Send [`LogMsg`]es to a server over TCP.
///
/// The messages are encoded and sent on separate threads
/// so that calling [`Client::send`] is non-blocking.
pub struct Client {
    msg_tx: Sender<MsgMsg>,
    flushed_rx: Receiver<FlushedMsg>,
    encode_quit_tx: Sender<QuitMsg>,
    send_quit_tx: Sender<InterruptMsg>,
    encode_join: Option<JoinHandle<()>>,
    send_join: Option<JoinHandle<()>>,

    /// Only used for diagnostics, not for communication after `new()`.
    addr: SocketAddr,
}

impl Client {
    /// Connect via TCP to this log server.
    ///
    /// `flush_timeout` is the minimum time the `TcpClient` will wait during a
    /// flush before potentially dropping data. Note: Passing `None` here can
    /// cause a call to `flush` to block indefinitely if a connection cannot be
    /// established.
    pub fn new(addr: SocketAddr, flush_timeout: Option<std::time::Duration>) -> Self {
        re_log::debug!("Connecting to remote {addr}…");

        // TODO(emilk): keep track of how much memory is in each pipe
        // and apply back-pressure to not use too much RAM.
        let (msg_tx, msg_rx) = crossbeam::channel::unbounded();
        let (packet_tx, packet_rx) = crossbeam::channel::unbounded();
        let (flushed_tx, flushed_rx) = crossbeam::channel::unbounded();
        let (encode_quit_tx, encode_quit_rx) = crossbeam::channel::unbounded();
        let (send_quit_tx, send_quit_rx) = crossbeam::channel::unbounded();

        // We don't compress the stream because we assume the SDK
        // and server are on the same machine and compression
        // can be expensive, see https://github.com/rerun-io/rerun/issues/2216
        let encoding_options = re_log_encoding::EncodingOptions::UNCOMPRESSED;

        let encode_join = std::thread::Builder::new()
            .name("msg_encoder".into())
            .spawn(move || {
                msg_encode(encoding_options, &msg_rx, &encode_quit_rx, &packet_tx);
            })
            .expect("Failed to spawn thread");

        let send_join = std::thread::Builder::new()
            .name("tcp_sender".into())
            .spawn(move || {
                tcp_sender(addr, flush_timeout, &packet_rx, &send_quit_rx, &flushed_tx);
            })
            .expect("Failed to spawn thread");

        Self {
            msg_tx,
            flushed_rx,
            encode_quit_tx,
            send_quit_tx,
            encode_join: Some(encode_join),
            send_join: Some(send_join),
            addr,
        }
    }

    pub fn send(&self, log_msg: LogMsg) {
        self.send_msg_msg(MsgMsg::LogMsg(log_msg));
    }

    /// Stall until all messages so far has been sent.
    pub fn flush(&self) {
        re_log::debug!("Flushing message queue…");
        if self.msg_tx.send(MsgMsg::Flush).is_err() {
            re_log::debug!("Flush failed: already shut down.");
            return;
        }

        match self.flushed_rx.recv() {
            Ok(FlushedMsg) => {
                re_log::debug!("Flush complete.");
            }
            Err(_) => {
                // This can happen on Ctrl-C
                re_log::warn!("Failed to flush pipeline - not all messages were sent.");
            }
        }
    }

    /// Switch to a mode where we drop messages if disconnected.
    ///
    /// Calling this before a flush (or drop) ensures we won't get stuck trying to send
    /// messages to a closed endpoint, but we will still send all messages to an open endpoint.
    pub fn drop_if_disconnected(&self) {
        self.send_quit_tx
            .send(InterruptMsg::DropIfDisconnected)
            .ok();
    }

    fn send_msg_msg(&self, msg: MsgMsg) {
        // ignoring errors, because Ctrl-C can shut down the receiving end.
        self.msg_tx.send(msg).ok();
    }
}

impl Drop for Client {
    /// Wait until everything has been sent.
    fn drop(&mut self) {
        re_log::debug!("Shutting down the client connection…");
        self.flush();
        // First shut down the encoder:
        self.encode_quit_tx.send(QuitMsg).ok();
        self.encode_join.take().map(|j| j.join().ok());
        // Then the other threads:
        self.send_quit_tx.send(InterruptMsg::Quit).ok();
        self.send_join.take().map(|j| j.join().ok());
        re_log::debug!("TCP client has shut down.");
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The other fields are all channels and join handles, so they are not usefully printable.
        f.debug_struct("Client")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

fn msg_encode(
    encoding_options: re_log_encoding::EncodingOptions,
    msg_rx: &Receiver<MsgMsg>,
    quit_rx: &Receiver<QuitMsg>,
    packet_tx: &Sender<PacketMsg>,
) {
    loop {
        select! {
            recv(msg_rx) -> msg_msg => {
                let Ok(msg_msg) = msg_msg else {
                    re_log::debug!("Shutting down msg_encode thread: channel has closed");
                    return; // channel has closed
                };

                let packet_msg = match &msg_msg {
                    MsgMsg::LogMsg(log_msg) => {
                        match re_log_encoding::encoder::encode_to_bytes(encoding_options, std::iter::once(log_msg)) {
                            Ok(packet) => {
                                re_log::trace!("Encoded message of size {}", packet.len());
                                Some(PacketMsg::Packet(packet))
                            }
                            Err(err) => {
                                re_log::error_once!("Failed to encode log message: {err}");
                                None
                            }
                        }
                    }
                    MsgMsg::Flush => Some(PacketMsg::Flush),
                };

                if let Some(packet_msg) = packet_msg {
                    if packet_tx.send(packet_msg).is_err() {
                        re_log::error!("Failed to send message to tcp_sender thread. Likely a shutdown race-condition.");
                        return;
                    }
                }
            }
            recv(quit_rx) -> _quit_msg => {
                re_log::debug!("Shutting down msg_encode thread: quit received");
                return;
            }
        }
    }
}

fn tcp_sender(
    addr: SocketAddr,
    flush_timeout: Option<std::time::Duration>,
    packet_rx: &Receiver<PacketMsg>,
    quit_rx: &Receiver<InterruptMsg>,
    flushed_tx: &Sender<FlushedMsg>,
) {
    let mut tcp_client = crate::tcp_client::TcpClient::new(addr, flush_timeout);
    // Once this flag has been set, we will drop all messages if the tcp_client is
    // no longer connected.
    let mut drop_if_disconnected = false;

    loop {
        select! {
            recv(packet_rx) -> packet_msg => {
                if let Ok(packet_msg) = packet_msg {
                    match packet_msg {
                        PacketMsg::Packet(packet) => {
                            match send_until_success(&mut tcp_client, drop_if_disconnected, &packet, quit_rx) {
                                Some(InterruptMsg::Quit) => {return;}
                                Some(InterruptMsg::DropIfDisconnected) => {
                                    drop_if_disconnected = true;
                                }
                                None => {}
                            }
                        }
                        PacketMsg::Flush => {
                            tcp_client.flush();
                            flushed_tx
                                .send(FlushedMsg)
                                .expect("Main thread should still be alive");
                        }
                    }
                } else {
                    re_log::debug!("Shutting down tcp_sender thread: packet_rx channel has closed");
                    return; // channel has closed
                }
            },
            recv(quit_rx) -> quit_msg => { match quit_msg {
                // Don't terminate on receiving a `DropIfDisconnected`. It's a soft-quit that allows
                // us to flush the pipeline.
                Ok(InterruptMsg::DropIfDisconnected) => {
                    drop_if_disconnected = true;
                }
                Ok(InterruptMsg::Quit) => {
                    re_log::debug!("Shutting down tcp_sender thread: received Quit message");
                    return;
                }
                Err(_) => {
                    re_log::debug!("Shutting down tcp_sender thread: quit_rx channel has closed");
                    return;
                }
            }}
        }
    }
}

fn send_until_success(
    tcp_client: &mut crate::tcp_client::TcpClient,
    drop_if_disconnected: bool,
    packet: &[u8],
    quit_rx: &Receiver<InterruptMsg>,
) -> Option<InterruptMsg> {
    // Early exit if tcp_client is disconnected
    if drop_if_disconnected && tcp_client.has_timed_out_for_flush() {
        re_log::warn_once!("Dropping messages because tcp client has timed out.");
        return None;
    }

    if let Err(err) = tcp_client.send(packet) {
        if drop_if_disconnected && tcp_client.has_timed_out_for_flush() {
            re_log::warn_once!("Dropping messages because tcp client has timed out.");
            return None;
        }
        // If this is the first time we fail to send the message, produce a warning.
        re_log::debug!("Failed to send message: {err}");

        let mut attempts = 1;
        let mut sleep_ms = 100;

        loop {
            select! {
                recv(quit_rx) -> _quit_msg => {
                    re_log::warn_once!("Dropping messages because tcp client has timed out or quitting.");
                    return Some(_quit_msg.unwrap_or(InterruptMsg::Quit));
                }
                default(std::time::Duration::from_millis(sleep_ms)) => {
                    if let Err(new_err) = tcp_client.send(packet) {
                        attempts += 1;
                        if attempts == 3 {
                            re_log::warn!("Failed to send message after {attempts} attempts: {err}");
                        }

                        if drop_if_disconnected && tcp_client.has_timed_out_for_flush() {
                            re_log::warn_once!("Dropping messages because tcp client has timed out.");
                            return None;
                        }

                        const MAX_SLEEP_MS : u64 = 3000;

                        sleep_ms = (sleep_ms * 2).min(MAX_SLEEP_MS);

                        // Only produce subsequent warnings once we've saturated the back-off
                        if sleep_ms == MAX_SLEEP_MS && new_err.to_string() != err.to_string() {
                            re_log::warn!("Still failing to send message after {attempts} attempts: {err}");
                        }
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
