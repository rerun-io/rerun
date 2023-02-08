use std::{
    io::Write,
    net::{SocketAddr, TcpStream},
};

/// State of the `TcpStream`
/// 
/// The `TcpStream` always starts in the Init state so that we can disambiguate
/// not having tried to connect from having failed to connect.
enum TcpStreamState {
    Init,
    Connected(TcpStream),
    Disconnected,
}

/// Connect to a rerun server and send log messages.
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
            stream_state: TcpStreamState::Init,
        }
    }

    pub fn set_addr(&mut self, addr: SocketAddr) {
        let addrs = vec![addr];
        if addrs != self.addrs {
            self.addrs = addrs;
            self.stream_state = TcpStreamState::Init;
        }
    }

    /// return `false` on failure. Does nothing if already connected.
    pub fn connect(&mut self) -> anyhow::Result<()> {
        if let TcpStreamState::Connected(_) = self.stream_state {
            Ok(())
        } else {
            re_log::debug!("Connecting to {:?}…", self.addrs);
            match TcpStream::connect(&self.addrs[..]) {
                Ok(mut stream) => {
                    if let Err(err) = stream.write(&crate::PROTOCOL_VERSION.to_le_bytes()) {
                        self.stream_state = TcpStreamState::Disconnected;
                        anyhow::bail!("Failed to send to Rerun server at {:?}: {err}", self.addrs);
                    } else {
                        self.stream_state = TcpStreamState::Connected(stream);
                        Ok(())
                    }
                }
                Err(err) => {
                    self.stream_state = TcpStreamState::Disconnected;
                    anyhow::bail!(
                        "Failed to connect to Rerun server at {:?}: {err}",
                        self.addrs
                    );
                }
            }
        }
    }

    /// blocks until it is sent
    pub fn send(&mut self, packet: &[u8]) -> anyhow::Result<()> {
        use std::io::Write as _;

        self.connect()?;

        if let TcpStreamState::Connected(stream) = &mut self.stream_state {
            re_log::trace!("Sending a packet of size {}…", packet.len());
            if let Err(err) = stream.write(&(packet.len() as u32).to_le_bytes()) {
                self.stream_state = TcpStreamState::Disconnected;
                anyhow::bail!("Failed to send to Rerun server at {:?}: {err}", self.addrs);
            }

            if let Err(err) = stream.write(packet) {
                self.stream_state = TcpStreamState::Disconnected;
                anyhow::bail!("Failed to send to Rerun server at {:?}: {err}", self.addrs);
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

    pub fn is_disconnected(&self) -> bool {
        match self.stream_state {
            TcpStreamState::Init | TcpStreamState::Connected(_) => false,
            TcpStreamState::Disconnected => true,
        }
    }
}
