use std::{
    io::Write,
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Failed to connect to Rerun server at {addr:?}: {err}")]
    Connect {
        addr: SocketAddr,
        err: std::io::Error,
    },

    #[error("Failed to send to Rerun server at {addr:?}: {err}")]
    Send {
        addr: SocketAddr,
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
    /// Tracks the duration and connection attempts since the last time the client
    /// was `Connected.`
    ///
    /// Behavior: Try to connect on next [`TcpClient::connect`] or [`TcpClient::send`].
    ///
    /// Transitions:
    ///  - Pending -> Connected on successful connection.
    ///  - Pending -> Pending on failed connection.
    Pending(Instant, usize),

    /// A healthy [`TcpStream`] ready to send packets
    ///
    /// Behavior: Send packets on [`TcpClient::send`]
    ///
    /// Transitions:
    ///  - Connected -> Disconnected on send error
    Connected(TcpStream),
}

impl TcpStreamState {
    fn reset() -> Self {
        Self::Pending(Instant::now(), 0)
    }
}

/// Connect to a rerun server and send log messages.
///
/// Blocking connection.
pub struct TcpClient {
    addr: SocketAddr,
    stream_state: TcpStreamState,
    disconnected_timeout: Option<Duration>,
}

impl TcpClient {
    pub fn new(addr: SocketAddr, disconnected_timeout: Option<Duration>) -> Self {
        Self {
            addr,
            stream_state: TcpStreamState::reset(),
            disconnected_timeout,
        }
    }

    /// Returns `false` on failure. Does nothing if already connected.
    ///
    /// [`Self::send`] will call this.
    pub fn connect(&mut self) -> Result<(), ClientError> {
        match self.stream_state {
            TcpStreamState::Connected(_) => Ok(()),
            TcpStreamState::Pending(since, tries) => {
                re_log::debug!("Connecting to {:?}…", self.addr);
                let timeout = std::time::Duration::from_secs(5);
                match TcpStream::connect_timeout(&self.addr, timeout) {
                    Ok(mut stream) => {
                        re_log::debug!("Connected to {:?}.", self.addr);
                        if let Err(err) = stream.write(&crate::PROTOCOL_VERSION.to_le_bytes()) {
                            self.stream_state = TcpStreamState::Pending(since, tries + 1);
                            Err(ClientError::Send {
                                addr: self.addr,
                                err,
                            })
                        } else {
                            self.stream_state = TcpStreamState::Connected(stream);
                            Ok(())
                        }
                    }
                    Err(err) => {
                        self.stream_state = TcpStreamState::Pending(since, tries + 1);
                        Err(ClientError::Connect {
                            addr: self.addr,
                            err,
                        })
                    }
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
                self.stream_state = TcpStreamState::reset();
                return Err(ClientError::Send {
                    addr: self.addr,
                    err,
                });
            }

            if let Err(err) = stream.write(packet) {
                self.stream_state = TcpStreamState::reset();
                return Err(ClientError::Send {
                    addr: self.addr,
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
        re_log::debug!("Flushing TCP stream…");
        match &mut self.stream_state {
            TcpStreamState::Pending(_, _) => {
                re_log::warn_once!(
                    "Tried to flush while TCP stream was still Pending. Data was possibly dropped."
                );
            }
            TcpStreamState::Connected(stream) => {
                if let Err(err) = stream.flush() {
                    re_log::warn!("Failed to flush: {err}");
                    self.stream_state = TcpStreamState::reset();
                }
            }
        }
        re_log::debug!("TCP stream flushed.");
    }

    /// Check if the underlying [`TcpStream`] is in the [`TcpStreamState::Pending`] state
    /// and has reached the timeout threshold.
    ///
    /// Note that this only occurs after a failure to connect or a failure to send.
    pub fn has_timed_out(&self) -> bool {
        match self.stream_state {
            TcpStreamState::Pending(since, tries) => {
                // If a timeout wasn't provided, never timeout
                self.disconnected_timeout.map_or(false, |timeout| {
                    Instant::now().duration_since(since) > timeout && tries > 0
                })
            }
            TcpStreamState::Connected(_) => false,
        }
    }
}
