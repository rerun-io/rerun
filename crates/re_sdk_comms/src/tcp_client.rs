use std::{
    io::Write,
    net::{SocketAddr, TcpStream},
};

/// Connect to a rerun server and send log messages.
pub struct TcpClient {
    addrs: Vec<SocketAddr>,
    stream: Option<TcpStream>,
}

impl Default for TcpClient {
    fn default() -> Self {
        Self::new(crate::default_server_addr())
    }
}

impl TcpClient {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addrs: vec![addr],
            stream: None,
        }
    }

    pub fn set_addr(&mut self, addr: SocketAddr) {
        self.addrs = vec![addr];
        self.stream = None;
    }

    /// blocks until it is sent
    pub fn send(&mut self, packet: &[u8]) {
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
            tracing::trace!("Sending a packet of size {}…", packet.len());
            if let Err(err) = stream.write(&(packet.len() as u32).to_le_bytes()) {
                tracing::warn!(
                    "Failed to send to Rerun server at {:?}: {err:?}",
                    self.addrs
                );
                self.stream = None;
                return;
            }

            if let Err(err) = stream.write(packet) {
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
