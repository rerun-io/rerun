use re_log_types::LogMsg;

/// Where the SDK sends its log messages.
pub trait LogSink: Send {
    /// Send this log message.
    fn send(&mut self, msg: LogMsg);

    /// Send all these log messages.
    fn send_all(&mut self, messages: Vec<LogMsg>) {
        for msg in messages {
            self.send(msg);
        }
    }

    /// Drain all buffered [`LogMsg`]es and return them.
    fn drain_backlog(&mut self) -> Vec<LogMsg> {
        vec![]
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    fn flush(&self) {}

    /// If the TCP session is disconnected, allow it to quit early and drop unsent messages.
    fn drop_msgs_if_disconnected(&self) {}
}

// ----------------------------------------------------------------------------

/// Store log messages in memory until you call [`LogSink::drain_backlog`].
#[derive(Default)]
pub struct BufferedSink(Vec<LogMsg>);

impl BufferedSink {
    pub fn new() -> Self {
        Self::default()
    }
}

impl LogSink for BufferedSink {
    fn send(&mut self, msg: LogMsg) {
        self.0.push(msg);
    }

    fn send_all(&mut self, mut messages: Vec<LogMsg>) {
        self.0.append(&mut messages);
    }

    fn drain_backlog(&mut self) -> Vec<LogMsg> {
        std::mem::take(&mut self.0)
    }
}

// ----------------------------------------------------------------------------

/// Stream log messages to a Rerun TCP server.
pub struct TcpSink {
    client: re_sdk_comms::Client,
}

impl TcpSink {
    pub fn new(addr: std::net::SocketAddr) -> Self {
        Self {
            client: re_sdk_comms::Client::new(addr),
        }
    }
}

impl LogSink for TcpSink {
    fn send(&mut self, msg: LogMsg) {
        self.client.send(msg);
    }

    fn flush(&self) {
        self.client.flush();
    }

    fn drop_msgs_if_disconnected(&self) {
        self.client.drop_if_disconnected();
    }
}

// ----------------------------------------------------------------------------

/// Stream log messages to a native viewer on the main thread.
#[cfg(feature = "native_viewer")]
pub struct NativeViewer(pub re_smart_channel::Sender<LogMsg>);

#[cfg(feature = "native_viewer")]
impl LogSink for NativeViewer {
    fn send(&mut self, msg: LogMsg) {
        if let Err(err) = self.0.send(msg) {
            re_log::error_once!("Failed to send log message to viewer: {err}");
        }
    }
}
