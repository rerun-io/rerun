use std::net::SocketAddr;

use re_log_types::{
    ApplicationId, ArrowMsg, BeginRecordingMsg, LogMsg, MsgId, PathOp, RecordingId, RecordingInfo,
    RecordingSource, Time, TimePoint,
};

use rerun::sink::LogSink;

// ----------------------------------------------------------------------------

/// Used to construct a [`RecordingInfo`]:
struct RecordingMetaData {
    recording_source: RecordingSource,
    application_id: Option<ApplicationId>,
    recording_id: RecordingId,
    is_official_example: Option<bool>,
}

impl Default for RecordingMetaData {
    fn default() -> Self {
        Self {
            // Will be filled in when we initialize the `rerun` python module.
            recording_source: RecordingSource::Unknown,
            application_id: Default::default(),
            // TODO(https://github.com/rerun-io/rerun/issues/1792): ZERO is not a great choice
            // here. Ideally we would use `default_recording_id(py)` instead.
            recording_id: RecordingId::ZERO,
            is_official_example: Default::default(),
        }
    }
}

impl RecordingMetaData {
    pub fn to_recording_info(&self) -> RecordingInfo {
        let recording_id = self.recording_id;

        let application_id = self
            .application_id
            .clone()
            .unwrap_or_else(ApplicationId::unknown);

        RecordingInfo {
            application_id,
            recording_id,
            is_official_example: self.is_official_example.unwrap_or(false),
            started: Time::now(),
            recording_source: self.recording_source.clone(),
        }
    }
}

// ----------------------------------------------------------------------------

/// The python API bindings create a single [`PythonSession`]
/// which is used to send log messages.
///
/// This mirrors the Python API to a certain extent, allowing users
/// to set enable/disable logging, set application id, switch log sinks, etc.
pub struct PythonSession {
    /// Is this session enabled?
    /// If not, all calls into it are ignored!
    enabled: bool,

    has_sent_begin_recording_msg: bool,
    recording_meta_data: RecordingMetaData,

    /// Where we put the log messages.
    sink: Box<dyn LogSink>,
}

impl Default for PythonSession {
    fn default() -> Self {
        let default_enabled = true;
        Self {
            enabled: rerun::decide_logging_enabled(default_enabled),
            has_sent_begin_recording_msg: false,
            recording_meta_data: Default::default(),
            sink: Box::new(rerun::sink::BufferedSink::new()),
        }
    }
}

impl PythonSession {
    pub fn set_python_version(&mut self, python_version: re_log_types::PythonVersion) {
        self.recording_meta_data.recording_source =
            re_log_types::RecordingSource::PythonSdk(python_version);
    }

    /// Check if logging is enabled on this `Session`.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable logging on this `Session`.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set whether or not logging is enabled by default.
    /// This will be overridden by the `RERUN` environment variable, if found.
    pub fn set_default_enabled(&mut self, default_enabled: bool) {
        self.enabled = rerun::decide_logging_enabled(default_enabled);
    }

    /// Set the [`ApplicationId`] to use for the following stream of log messages.
    ///
    /// This should be called once before anything else.
    /// If you don't call this, the resulting application id will be [`ApplicationId::unknown`].
    ///
    /// Note that many recordings can share the same [`ApplicationId`], but
    /// they all have unique [`RecordingId`]s.
    pub fn set_application_id(&mut self, application_id: ApplicationId, is_official_example: bool) {
        if self.recording_meta_data.application_id.as_ref() != Some(&application_id) {
            self.recording_meta_data.application_id = Some(application_id);
            self.recording_meta_data.is_official_example = Some(is_official_example);
            self.has_sent_begin_recording_msg = false;
        }
    }

    /// The current [`RecordingId`], if set.
    pub fn recording_id(&self) -> RecordingId {
        self.recording_meta_data.recording_id
    }

    /// Set the [`RecordingId`] of this message stream.
    ///
    /// If you're logging from multiple processes and want all the messages
    /// to end up as the same recording, you must make sure they all set the same
    /// [`RecordingId`] using this function.
    ///
    /// Note that many recordings can share the same [`ApplicationId`], but
    /// they all have unique [`RecordingId`]s.
    pub fn set_recording_id(&mut self, recording_id: RecordingId) {
        if self.recording_meta_data.recording_id != recording_id {
            self.recording_meta_data.recording_id = recording_id;
            self.has_sent_begin_recording_msg = false;
        }
    }

    /// Set the [`LogSink`] to use. This is where the log messages will be sent.
    ///
    /// If the previous sink is [`rerun::sink::BufferedSink`] (the default),
    /// it will be drained and sent to the new sink.
    pub fn set_sink(&mut self, sink: Box<dyn LogSink>) {
        let backlog = self.sink.drain_backlog();
        self.sink = sink;
        self.sink.send_all(backlog);
    }

    /// Send log data to a remote viewer/server.
    ///
    /// Usually this is done by running the `rerun` binary (`cargo install rerun`) without arguments,
    /// and then connecting to it.
    ///
    /// Send all currently buffered messages.
    /// If we are already connected, we will re-connect to this new address.
    ///
    /// This function returns immediately.
    /// Disconnect with [`Self::disconnect`].
    pub fn connect(&mut self, addr: SocketAddr) {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to connect() ignored");
            return;
        }

        re_log::debug!("Connecting to remote {addr}…");
        self.set_sink(Box::new(rerun::sink::TcpSink::new(addr)));
    }

    /// Drains all pending log messages and saves them to disk into an rrd file.
    pub fn save(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<(), rerun::sink::FileSinkError> {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to save() ignored");
            return Ok(());
        }

        self.set_sink(Box::new(rerun::sink::FileSink::new(path)?));
        Ok(())
    }

    /// Disconnects any TCP connection, shuts down any server, and closes any file.
    pub fn disconnect(&mut self) {
        self.set_sink(Box::new(rerun::sink::BufferedSink::new()));
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    pub fn flush(&mut self) {
        self.sink.flush();
    }

    /// If the tcp session is disconnected, allow it to quit early and drop unsent messages
    pub fn drop_msgs_if_disconnected(&mut self) {
        self.sink.drop_msgs_if_disconnected();
    }

    /// Send a [`LogMsg`].
    pub fn send(&mut self, log_msg: LogMsg) {
        if !self.enabled {
            // It's intended that the logging SDK should drop messages earlier than this if logging is disabled. This
            // check here is just a safety net.
            re_log::debug_once!("Logging is disabled, dropping message.");
            return;
        }

        if !self.has_sent_begin_recording_msg {
            let info = self.recording_meta_data.to_recording_info();

            re_log::debug!(
                "Beginning new recording with application_id {:?} and recording id {}",
                info.application_id.0,
                info.recording_id
            );

            self.sink.send(
                BeginRecordingMsg {
                    msg_id: MsgId::random(),
                    info,
                }
                .into(),
            );
            self.has_sent_begin_recording_msg = true;
        }

        self.sink.send(log_msg);
    }

    pub fn send_arrow_msg(&mut self, arrow_msg: ArrowMsg) {
        self.send(LogMsg::ArrowMsg(self.recording_id(), arrow_msg));
    }

    /// Send a [`PathOp`].
    pub fn send_path_op(&mut self, time_point: &TimePoint, path_op: PathOp) {
        self.send(LogMsg::EntityPathOpMsg(
            self.recording_id(),
            re_log_types::EntityPathOpMsg {
                msg_id: MsgId::random(),
                time_point: time_point.clone(),
                path_op,
            },
        ));
    }
}
