use std::{
    io::Write,
    net::{SocketAddr, TcpStream},
};

use re_log_types::LogMsg;

/// Connect to a rerun server and send log messages.
pub struct Client {
    addrs: Vec<SocketAddr>,
    stream: Option<TcpStream>,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            addrs: vec![crate::default_server_addr()],
            stream: None,
        }
    }
}

impl Client {
    pub fn set_addr(&mut self, addr: SocketAddr) {
        self.addrs = vec![addr];
        self.stream = None;
    }

    pub fn send(&mut self, log_msg: &LogMsg) {
        use std::io::Write as _;

        if self.stream.is_none() {
            match TcpStream::connect(&self.addrs[..]) {
                Ok(mut stream) => {
                    // `set_nonblocking(true)` will make the messages not all arrive, which is bad.
                    // stream
                    //     .set_nonblocking(true)
                    //     .expect("set_nonblocking call failed");

                    // stream
                    //     .set_nodelay(true)
                    //     .expect("Couldn't disable Nagle's algorithm");

                    if let Err(err) = stream.write(&crate::PROTOCOL_VERSION.to_le_bytes()) {
                        tracing::warn!(
                            "Failed to send to Rerun server at {:?}: {err:?}",
                            self.addrs
                        );
                    } else {
                        self.stream = Some(stream);
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to connect to Rerun server at {:?}: {err:?}",
                        self.addrs
                    );
                }
            }
        }

        if let Some(stream) = &mut self.stream {
            let msg = crate::encode_log_msg(log_msg);

            tracing::trace!("Sending a LogMsg of size {}…", msg.len());
            if let Err(err) = stream.write(&(msg.len() as u32).to_le_bytes()) {
                tracing::warn!(
                    "Failed to send to Rerun server at {:?}: {err:?}",
                    self.addrs
                );
                self.stream = None;
                return;
            }

            if let Err(err) = stream.write(&msg) {
                tracing::warn!(
                    "Failed to send to Rerun server at {:?}: {err:?}",
                    self.addrs
                );
                self.stream = None;
            }
        }
    }

    /// Wait until all logged data have been sent.
    pub fn flush(&mut self) {
        if let Some(stream) = &mut self.stream {
            if let Err(err) = stream.flush() {
                tracing::warn!("Failed to flush: {:?}", err);
            }
        }
        tracing::trace!("TCP stream flushed.");
    }
}
