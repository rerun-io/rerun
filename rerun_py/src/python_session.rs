use std::net::SocketAddr;

use pyo3::{exceptions::PyValueError, PyResult};
use re_log_types::{
    ApplicationId, ArrowMsg, BeginRecordingMsg, DataRow, DataTableError, LogMsg, PathOp,
    RecordingId, RecordingInfo, RecordingSource, RowId, Time, TimePoint,
};

use depthai_viewer::sink::LogSink;
#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
// ----------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum PythonSessionError {
    #[allow(dead_code)]
    #[error("The Rerun SDK was not compiled with the '{0}' feature")]
    FeatureNotEnabled(&'static str),

    #[cfg(feature = "web_viewer")]
    #[error("Could not start the WebViewerServer: '{0}'")]
    WebViewerServerError(#[from] re_web_viewer_server::WebViewerServerError),
}

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

    build_info: re_build_info::BuildInfo,

    /// Used to serve the web viewer assets.
    /// TODO(jleibs): Potentially use this for serve as well
    #[cfg(feature = "web_viewer")]
    web_viewer_server: Option<re_web_viewer_server::WebViewerServerHandle>,
}

impl Default for PythonSession {
    fn default() -> Self {
        let default_enabled = true;
        Self {
            enabled: depthai_viewer::decide_logging_enabled(default_enabled),
            has_sent_begin_recording_msg: false,
            recording_meta_data: Default::default(),
            sink: Box::new(depthai_viewer::sink::BufferedSink::new()),
            build_info: re_build_info::build_info!(),
            #[cfg(feature = "web_viewer")]
            web_viewer_server: None,
        }
    }
}

type SysExePath = String;

impl PythonSession {
    pub fn set_python_version(
        &mut self,
        python_version: re_log_types::PythonVersion,
        sys_exe: SysExePath,
    ) {
        self.recording_meta_data.recording_source =
            re_log_types::RecordingSource::PythonSdk(python_version, sys_exe, String::new());
    }

    /// Check if logging is enabled on this `Session`.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn version(&self) -> String {
        let build_info = re_build_info::build_info!();
        build_info.version.to_string()
    }

    /// Enable or disable logging on this `Session`.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set whether or not logging is enabled by default.
    /// This will be overridden by the `RERUN` environment variable, if found.
    pub fn set_default_enabled(&mut self, default_enabled: bool) {
        self.enabled = depthai_viewer::decide_logging_enabled(default_enabled);
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

    /// The current [`RecordingId`].
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
    /// If the previous sink is [`depthai_viewer::sink::BufferedSink`] (the default),
    /// it will be drained and sent to the new sink.
    pub fn set_sink(&mut self, sink: Box<dyn LogSink>) {
        // Capture the backlog (should only apply if this was a `BufferedSink`)
        let backlog = self.sink.drain_backlog();

        // Before changing the sink, we set drop_if_disconnected and
        // flush. This ensures that any messages that are currently
        // buffered will be sent.
        self.sink.drop_msgs_if_disconnected();
        self.sink.flush();
        self.sink = sink;

        if backlog.is_empty() {
            // If we had no backlog, we need to send the `BeginRecording` message to the new sink.
            self.has_sent_begin_recording_msg = false;
        } else {
            // Otherwise the backlog should have had the `BeginRecording` message in it already.
            self.sink.send_all(backlog);
        }
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
        self.set_sink(Box::new(depthai_viewer::sink::TcpSink::new(addr)));
    }

    /// Send all pending and future log messages to disk as an rrd file
    pub fn save(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<(), depthai_viewer::sink::FileSinkError> {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to save() ignored");
            return Ok(());
        }

        self.set_sink(Box::new(depthai_viewer::sink::FileSink::new(path)?));
        Ok(())
    }

    /// Send all pending and future log messages to an in-memory store
    pub fn memory_recording(&mut self) -> depthai_viewer::sink::MemorySinkStorage {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to memory_recording() ignored");
            return Default::default();
        }

        let memory_sink = depthai_viewer::sink::MemorySink::default();
        let buffer = memory_sink.buffer();

        self.set_sink(Box::new(memory_sink));
        self.has_sent_begin_recording_msg = false;

        buffer
    }

    /// Disconnects any TCP connection, shuts down any server, and closes any file.
    pub fn disconnect(&mut self) {
        self.set_sink(Box::new(depthai_viewer::sink::BufferedSink::new()));
        self.has_sent_begin_recording_msg = false;
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    pub fn flush(&mut self) {
        self.sink.flush();
    }

    /// Send a single [`DataRow`].
    pub fn send_row(&mut self, row: DataRow) -> PyResult<()> {
        let msg = row
            .into_table()
            .to_arrow_msg()
            .map_err(|err: DataTableError| PyValueError::new_err(err.to_string()))?;

        self.send(LogMsg::ArrowMsg(self.recording_id(), msg));

        Ok(())
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

            // This shouldn't happen, but at least log an error if it does.
            // See: https://github.com/rerun-io/rerun/issues/1792
            if info.recording_id == RecordingId::ZERO {
                re_log::error_once!("RecordingId was still ZERO when sent to server. This is a python initialization bug.");
            }

            re_log::debug!(
                "Beginning new recording with application_id {:?} and recording id {}",
                info.application_id.0,
                info.recording_id
            );

            self.sink.send(
                BeginRecordingMsg {
                    row_id: RowId::random(),
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
                row_id: RowId::random(),
                time_point: time_point.clone(),
                path_op,
            },
        ));
    }

    /// Get a url to an instance of the web-viewer
    ///
    /// This may point to app.rerun.io or localhost depending on
    /// whether `host_assets` was called.
    pub fn get_app_url(&self) -> String {
        #[cfg(feature = "web_viewer")]
        if let Some(hosted_assets) = &self.web_viewer_server {
            return format!("http://localhost:{}", hosted_assets.port());
        }

        let short_git_hash = &self.build_info.git_hash[..7];
        format!("https://app.rerun.io/commit/{short_git_hash}")
    }

    /// Start a web server to host the run web-asserts
    ///
    /// The caller needs to ensure that there is a `tokio` runtime running.
    #[allow(clippy::unnecessary_wraps)]
    #[cfg(feature = "web_viewer")]
    pub fn start_web_viewer_server(
        &mut self,
        _web_port: WebViewerServerPort,
    ) -> Result<(), PythonSessionError> {
        self.web_viewer_server = Some(re_web_viewer_server::WebViewerServerHandle::new(_web_port)?);

        Ok(())
    }
}
