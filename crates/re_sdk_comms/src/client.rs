use std::net::{SocketAddr, TcpStream};

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
                Ok(stream) => {
                    self.stream = Some(stream);
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
            if let Err(err) = stream.write(&msg) {
                tracing::warn!(
                    "Failed to send to Rerun server at {:?}: {err:?}",
                    self.addrs
                );
                self.stream = None;
            }
        }
    }
}
