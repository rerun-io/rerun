use std::{
    io::Write,
    net::{SocketAddr, TcpStream},
};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Failed to connect to Rerun server at {addrs:?}: {err}")]
    Connect {
        addrs: Vec<SocketAddr>,
        err: std::io::Error,
    },

    #[error("Failed to send to Rerun server at {addrs:?}: {err}")]
    Send {
        addrs: Vec<SocketAddr>,
        err: std::io::Error,
    },
}

/// State of the [`TcpStream`]
///
/// Because the [`TcpClient`] lazily connects on [`TcpClient::send`], it needs a
/// very simple state machine to track the state of the connection. A trinary
/// state is used to specifically differentiate between
/// [`TcpStreamState::Pending`] which is still a nominal state for any new tcp
/// connection, and [`TcpStreamState::Disconnected`] which implies either a
/// failure to connect, or an error on an already established stream.
enum TcpStreamState {
    /// The [`TcpStream`] is yet to be connected.
    ///
    /// Behavior: Try to connect on next [`TcpClient::connect`] or [`TcpClient::send`].
    ///
    /// Transitions:
    ///  - Pending -> Connected on successful connection.
    ///  - Pending -> Disconnected on failed connection.
    Pending,

    /// A healthy [`TcpStream`] ready to send packets
    ///
    /// Behavior: Send packets on [`TcpClient::send`]
    ///
    /// Transitions:
    ///  - Connected -> Disconnected on send error
    Connected(TcpStream),

    /// A broken [`TcpStream`] which experienced a failure to connect or send.
    ///
    /// Behavior: Try to re-connect on next [`TcpClient::connect`] or [`TcpClient::send`]
    ///
    /// Transitions:
    ///  - Disconnected -> Connected on successful connection.
    Disconnected,
}

/// Connect to a rerun server and send log messages.
///
/// Blocking connection.
pub struct TcpClient {
    addrs: Vec<SocketAddr>,
    stream_state: TcpStreamState,
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
            stream_state: TcpStreamState::Pending,
        }
    }

    /// Returns `false` on failure. Does nothing if already connected.
    ///
    /// [`Self::send`] will call this.
    pub fn connect(&mut self) -> Result<(), ClientError> {
        if let TcpStreamState::Connected(_) = self.stream_state {
            Ok(())
        } else {
            re_log::debug!("Connecting to {:?}…", self.addrs);
            match TcpStream::connect(&self.addrs[..]) {
                Ok(mut stream) => {
                    if let Err(err) = stream.write(&crate::PROTOCOL_VERSION.to_le_bytes()) {
                        self.stream_state = TcpStreamState::Disconnected;
                        Err(ClientError::Send {
                            addrs: self.addrs.clone(),
                            err,
                        })
                    } else {
                        self.stream_state = TcpStreamState::Connected(stream);
                        Ok(())
                    }
                }
                Err(err) => {
                    self.stream_state = TcpStreamState::Disconnected;
                    Err(ClientError::Connect {
                        addrs: self.addrs.clone(),
                        err,
                    })
                }
            }
        }
    }

    /// Blocks until it is sent.
    pub fn send(&mut self, packet: &[u8]) -> Result<(), ClientError> {
        use std::io::Write as _;

        self.connect()?;

        if let TcpStreamState::Connected(stream) = &mut self.stream_state {
            re_log::trace!("Sending a packet of size {}…", packet.len());
            if let Err(err) = stream.write(&(packet.len() as u32).to_le_bytes()) {
                self.stream_state = TcpStreamState::Disconnected;
                return Err(ClientError::Send {
                    addrs: self.addrs.clone(),
                    err,
                });
            }

            if let Err(err) = stream.write(packet) {
                self.stream_state = TcpStreamState::Disconnected;
                return Err(ClientError::Send {
                    addrs: self.addrs.clone(),
                    err,
                });
            }

            Ok(())
        } else {
            unreachable!("self.connect should have ensured this");
        }
    }

    /// Wait until all logged data have been sent.
    pub fn flush(&mut self) {
        if let TcpStreamState::Connected(stream) = &mut self.stream_state {
            if let Err(err) = stream.flush() {
                re_log::warn!("Failed to flush: {err}");
                self.stream_state = TcpStreamState::Disconnected;
            }
        }
        re_log::trace!("TCP stream flushed.");
    }

    /// Check if the underlying [`TcpStream`] has entered the [`TcpStreamState::Disconnected`] state
    ///
    /// Note that this only occurs after a failure to connect or a failure to send.
    pub fn has_disconnected(&self) -> bool {
        match self.stream_state {
            TcpStreamState::Pending | TcpStreamState::Connected(_) => false,
            TcpStreamState::Disconnected => true,
        }
    }
}
