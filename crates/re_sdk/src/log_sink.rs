use re_log_types::LogMsg;

/// The sink for all log messages from the SDK.
pub enum LogSink {
    Buffered(Vec<LogMsg>),

    File(crate::file_writer::FileWriter),

    Remote(re_sdk_comms::Client),

    #[cfg(feature = "native_viewer")]
    NativeViewer(re_smart_channel::Sender<LogMsg>),

    /// Serve it to the web viewer over WebSockets
    #[cfg(feature = "web_viewer")]
    WebViewer(crate::remote_viewer_server::RemoteViewerServer),
}

impl Default for LogSink {
    fn default() -> Self {
        LogSink::Buffered(vec![])
    }
}

impl LogSink {
    /// Drain all buffered [`LogMsg`]es and return them.
    pub fn drain_backlog(&mut self) -> Vec<LogMsg> {
        if let Self::Buffered(log_messages) = self {
            std::mem::take(log_messages)
        } else {
            vec![]
        }
    }

    pub fn send(&mut self, msg: LogMsg) {
        match self {
            Self::Buffered(buffer) => buffer.push(msg),

            Self::File(file) => file.write(msg),

            Self::Remote(client) => client.send(msg),

            #[cfg(feature = "native_viewer")]
            Self::NativeViewer(sender) => {
                if let Err(err) = sender.send(msg) {
                    re_log::error_once!("Failed to send log message to viewer: {err}");
                }
            }

            #[cfg(feature = "web_viewer")]
            Self::WebViewer(remote) => {
                remote.send(msg);
            }
        }
    }

    pub fn send_all(&mut self, mut messages: Vec<LogMsg>) {
        match self {
            Self::Buffered(buffer) => buffer.append(&mut messages),
            _ => {
                for msg in messages {
                    self.send(msg);
                }
            }
        }
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    pub fn flush(&self) {
        if let LogSink::Remote(sender) = self {
            sender.flush();
        }
    }

    /// If the tcp session is disconnected, allow it to quit early and drop unsent messages
    pub fn drop_msgs_if_disconnected(&self) {
        if let LogSink::Remote(sender) = self {
            sender.drop_if_disconnected();
        }
    }
}
