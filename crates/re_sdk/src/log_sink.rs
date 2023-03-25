use re_log_types::LogMsg;

/// Where the SDK sends its log messages.
pub trait LogSink: Send + Sync + 'static {
    /// Send this log message.
    fn send(&self, msg: LogMsg);

    /// Send all these log messages.
    fn send_all(&self, messages: Vec<LogMsg>) {
        for msg in messages {
            self.send(msg);
        }
    }

    /// Drain all buffered [`LogMsg`]es and return them.
    fn drain_backlog(&self) -> Vec<LogMsg> {
        vec![]
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    fn flush(&self) {}

    /// If the TCP session is disconnected, allow it to quit early and drop unsent messages.
    fn drop_msgs_if_disconnected(&self) {}

    /// Returns `false` if this sink just discards all messages.
    fn is_enabled(&self) -> bool {
        true
    }
}

// ----------------------------------------------------------------------------

struct DisabledSink;

impl LogSink for DisabledSink {
    fn send(&self, _msg: LogMsg) {
        // It's intended that the logging SDK should drop messages earlier than this if logging is disabled.
        re_log::debug_once!("Logging is disabled, dropping message(s).");
    }

    fn is_enabled(&self) -> bool {
        false
    }
}

/// A sink that does nothing. All log messages are just dropped.
pub fn disabled() -> Box<dyn LogSink> {
    Box::new(DisabledSink)
}

// ----------------------------------------------------------------------------

/// Store log messages in memory until you call [`LogSink::drain_backlog`].
#[derive(Default)]
pub struct BufferedSink(parking_lot::Mutex<Vec<LogMsg>>);

impl BufferedSink {
    /// An empty buffer.
    pub fn new() -> Self {
        Self::default()
    }
}

impl LogSink for BufferedSink {
    fn send(&self, msg: LogMsg) {
        self.0.lock().push(msg);
    }

    fn send_all(&self, mut messages: Vec<LogMsg>) {
        self.0.lock().append(&mut messages);
    }

    fn drain_backlog(&self) -> Vec<LogMsg> {
        std::mem::take(&mut self.0.lock())
    }
}

// ----------------------------------------------------------------------------

/// Stream log messages to a Rerun TCP server.
pub struct TcpSink {
    client: re_sdk_comms::Client,
}

impl TcpSink {
    /// Connect to the given address in a background thread.
    /// Retries until successful.
    pub fn new(addr: std::net::SocketAddr) -> Self {
        Self {
            client: re_sdk_comms::Client::new(addr),
        }
    }
}

impl LogSink for TcpSink {
    fn send(&self, msg: LogMsg) {
        self.client.send(msg);
    }

    fn flush(&self) {
        self.client.flush();
    }

    fn drop_msgs_if_disconnected(&self) {
        self.client.drop_if_disconnected();
    }
}
