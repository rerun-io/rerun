use std::net::SocketAddr;

use re_log_types::{
    ApplicationId, BeginRecordingMsg, LogMsg, MsgId, PathOp, RecordingId, RecordingInfo,
    RecordingSource, Time, TimePoint,
};

use crate::{file_writer::FileWriter, LogSink};

/// Used to contruct a [`RecordingInfo`]:
struct RecordingMetaData {
    recording_source: RecordingSource,
    application_id: Option<ApplicationId>,
    recording_id: Option<RecordingId>,
    is_official_example: Option<bool>,
}

impl Default for RecordingMetaData {
    fn default() -> Self {
        Self {
            recording_source: RecordingSource::RustSdk {
                rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            },
            application_id: Default::default(),
            recording_id: Default::default(),
            is_official_example: Default::default(),
        }
    }
}

impl RecordingMetaData {
    pub fn to_recording_info(&self) -> Option<RecordingInfo> {
        let recording_id = self.recording_id?;

        let application_id = self
            .application_id
            .clone()
            .unwrap_or_else(ApplicationId::unknown);

        re_log::debug!(
            "Beginning new recording with application_id {:?} and recording id {}",
            application_id.0,
            recording_id
        );

        Some(RecordingInfo {
            application_id,
            recording_id,
            is_official_example: self.is_official_example.unwrap_or_default(),
            started: Time::now(),
            recording_source: self.recording_source.clone(),
        })
    }
}

/// This is the main object you need to create to use the Rerun SDK.
///
/// You should ideally create one session object and reuse it.
/// For convenience, there is a global [`Session`] object you can access with [`crate::global_session`].
pub struct Session {
    /// Is this session enabled?
    /// If not, all calls into it are ignored!
    enabled: bool,

    has_sent_begin_recording_msg: bool,
    recording_meta_data: RecordingMetaData,

    // Used by `rerun::serve_web_viewer`
    #[cfg(all(feature = "tokio_runtime", not(target_arch = "wasm32")))]
    tokio_runtime: tokio::runtime::Runtime,

    /// Where we put the log messages.
    sink: Box<dyn LogSink>,
}

impl Default for Session {
    fn default() -> Self {
        Self::with_default_enabled(true)
    }
}

impl Session {
    /// Initializes a new session with a properly set [`ApplicationId`], [`RecordingId`] and
    /// logging toggle.
    /// This is a higher-level interface on top of [`Self::new`] and
    /// [`Self::with_default_enabled`].
    ///
    /// `default_enabled` controls whether or not logging is enabled by default.
    /// The default can always be overridden using the `RERUN` environment variable
    /// or by calling [`Self::set_enabled`].
    ///
    /// Usually you should only call this once and then reuse the same [`Session`].
    #[track_caller]
    pub fn init(application_id: impl Into<ApplicationId>, default_enabled: bool) -> Self {
        let is_official_example = called_from_official_rust_example();

        let mut session = Self::with_default_enabled(default_enabled);
        session.set_application_id(application_id.into(), is_official_example);
        session.set_recording_id(RecordingId::random());

        session
    }

    /// Construct a new session.
    ///
    /// Usually you should only call this once and then reuse the same [`Session`].
    ///
    /// For convenience, there is also a global [`Session`] object you can access with
    /// [`crate::global_session`].
    ///
    /// Logging is enabled by default, but can be turned off with the `RERUN` environment variable
    /// or by calling [`Self::set_enabled`].
    #[doc(hidden)]
    pub fn new() -> Self {
        Self::with_default_enabled(true)
    }

    /// Construct a new session, with control of whether or not logging is enabled by default.
    ///
    /// The default can always be overridden using the `RERUN` environment variable
    /// or by calling [`Self::set_enabled`].
    #[doc(hidden)]
    pub fn with_default_enabled(default_enabled: bool) -> Self {
        let enabled = crate::decide_logging_enabled(default_enabled);

        Self {
            enabled,

            has_sent_begin_recording_msg: false,
            recording_meta_data: Default::default(),

            #[cfg(all(feature = "tokio_runtime", not(target_arch = "wasm32")))]
            tokio_runtime: tokio::runtime::Runtime::new().unwrap(),

            sink: Box::new(crate::log_sink::BufferedSink::new()),
        }
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
        self.enabled = crate::decide_logging_enabled(default_enabled);
    }

    /// Used by the `rerun` crate when hosting a web viewer server.
    #[doc(hidden)]
    #[cfg(all(feature = "tokio_runtime", not(target_arch = "wasm32")))]
    pub fn tokio_runtime(&self) -> &tokio::runtime::Runtime {
        &self.tokio_runtime
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
    pub fn recording_id(&self) -> Option<RecordingId> {
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
        if self.recording_meta_data.recording_id != Some(recording_id) {
            self.recording_meta_data.recording_id = Some(recording_id);
            self.has_sent_begin_recording_msg = false;
        }
    }

    /// Set where the recording is coming from.
    /// The default is [`RecordingSource::RustSdk`].
    pub fn set_recording_source(&mut self, recording_source: RecordingSource) {
        self.recording_meta_data.recording_source = recording_source;
    }

    /// Where the recording is coming from.
    /// The default is [`RecordingSource::RustSdk`].
    pub fn recording_source(&self) -> &RecordingSource {
        &self.recording_meta_data.recording_source
    }

    /// Set the [`LogSink`] to use. This is where the log messages will be sent.
    ///
    /// If the previous sink is [`crate::log_sink::BufferedSink`] (the default),
    /// it will be drained and sent to the new sink.
    pub fn set_sink(&mut self, sink: Box<dyn LogSink>) {
        let backlog = self.sink.drain_backlog();
        self.sink = sink;
        self.sink.send_all(backlog);
    }

    /// Drain all buffered [`LogMsg`]es and return them.
    pub fn drain_backlog(&mut self) -> Vec<LogMsg> {
        self.sink.drain_backlog()
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
    ///
    /// ## Example:
    ///
    /// ``` no_run
    /// # let mut session = re_sdk::Session::new();
    /// session.connect(re_sdk::default_server_addr());
    /// ```
    pub fn connect(&mut self, addr: SocketAddr) {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to connect() ignored");
            return;
        }

        re_log::debug!("Connecting to remote {addr}…");
        self.set_sink(Box::new(crate::log_sink::TcpSink::new(addr)));
    }

    /// Disconnects any TCP connection, shuts down any server, and closes any file.
    pub fn disconnect(&mut self) {
        self.set_sink(Box::new(crate::log_sink::BufferedSink::new()));
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
            if let Some(info) = self.recording_meta_data.to_recording_info() {
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
        }

        self.sink.send(log_msg);
    }

    /// Send a [`PathOp`].
    pub fn send_path_op(&mut self, time_point: &TimePoint, path_op: PathOp) {
        self.send(LogMsg::EntityPathOpMsg(re_log_types::EntityPathOpMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            path_op,
        }));
    }

    /// Drains all pending log messages and saves them to disk into an rrd file.
    // TODO(cmc): We're gonna have to properly type all these errors all the way up to the encoding
    // methods in re_log_types at some point...
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&mut self, path: impl Into<std::path::PathBuf>) -> anyhow::Result<()> {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to save() ignored");
            return Ok(());
        }

        self.set_sink(Box::new(FileWriter::new(path)?));
        Ok(())
    }
}

#[track_caller]
fn called_from_official_rust_example() -> bool {
    // The sentinel file we use to identify the official examples directory.
    const SENTINEL_FILENAME: &str = ".rerun_examples";
    let caller = core::panic::Location::caller();
    let mut path = std::path::PathBuf::from(caller.file());
    let mut is_official_example = false;
    for _ in 0..4 {
        path.pop(); // first iteration is always a file path in our examples
        if path.join(SENTINEL_FILENAME).exists() {
            is_official_example = true;
        }
    }
    is_official_example
}
