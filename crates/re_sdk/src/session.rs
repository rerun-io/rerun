use std::sync::Arc;

use re_log_types::{ApplicationId, LogMsg, RecordingId, RecordingInfo, RecordingSource, Time};

use crate::sink::LogSink;

// ----------------------------------------------------------------------------

/// Construct a [`Session`].
///
/// ``` no_run
/// # use re_sdk::SessionBuilder;
/// let session = SessionBuilder::new("my_app").save("my_recording.rrd")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub struct SessionBuilder {
    application_id: ApplicationId,
    is_official_example: bool,
    enabled: Option<bool>,
    default_enabled: bool,
    recording_id: Option<RecordingId>,
}

impl SessionBuilder {
    /// Create a new [`SessionBuilder`] with an application id.
    ///
    /// The application id is usually the name of your app.
    ///
    /// ``` no_run
    /// # use re_sdk::SessionBuilder;
    /// let session = SessionBuilder::new("my_app").save("my_recording.rrd")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[track_caller] // track_caller so that we can see if we are being called from an official example.
    pub fn new(application_id: impl Into<ApplicationId>) -> Self {
        let application_id = application_id.into();
        let is_official_example = crate::called_from_official_rust_example();

        Self {
            application_id,
            is_official_example,
            enabled: None,
            default_enabled: true,
            recording_id: None,
        }
    }

    /// Set whether or not Rerun is enabled by default.
    ///
    /// If the `RERUN` environment variable is set, it will override this.
    ///
    /// Set also: [`Self::enabled`].
    pub fn default_enabled(mut self, default_enabled: bool) -> Self {
        self.default_enabled = default_enabled;
        self
    }

    /// Set whether or not Rerun is enabled.
    ///
    /// Setting this will ignore the `RERUN` environment variable.
    ///
    /// Set also: [`Self::default_enabled`].
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Set the [`RecordingId`] for this session.
    ///
    /// If you're logging from multiple processes and want all the messages
    /// to end up as the same recording, you must make sure they all set the same
    /// [`RecordingId`] using this function.
    ///
    /// Note that many recordings can share the same [`ApplicationId`], but
    /// they all have unique [`RecordingId`]s.
    ///
    /// The default is to use a random [`RecordingId`].
    pub fn recording_id(mut self, recording_id: RecordingId) -> Self {
        self.recording_id = Some(recording_id);
        self
    }

    /// Buffer log messages in RAM.
    ///
    /// Retrieve them later with [`Session::drain_backlog`].
    pub fn buffered(self) -> Session {
        let (rerun_enabled, recording_info) = self.finalize();
        if rerun_enabled {
            Session::buffered(recording_info)
        } else {
            re_log::debug!("Rerun disabled - call to buffered() ignored");
            Session::disabled()
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
    ///
    /// ## Example:
    ///
    /// ``` no_run
    /// let session = re_sdk::SessionBuilder::new("my_app").connect(re_sdk::default_server_addr());
    /// ```
    pub fn connect(self, addr: std::net::SocketAddr) -> Session {
        let (rerun_enabled, recording_info) = self.finalize();
        if rerun_enabled {
            Session::new(
                recording_info,
                Box::new(crate::log_sink::TcpSink::new(addr)),
            )
        } else {
            re_log::debug!("Rerun disabled - call to connect() ignored");
            Session::disabled()
        }
    }

    /// Stream all log messages to an `.rrd` file.
    ///
    /// ``` no_run
    /// # use re_sdk::SessionBuilder;
    /// let session = SessionBuilder::new("my_app").save("my_recording.rrd")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(self, path: impl Into<std::path::PathBuf>) -> anyhow::Result<Session> {
        let (rerun_enabled, recording_info) = self.finalize();
        if rerun_enabled {
            Ok(Session::new(
                recording_info,
                Box::new(crate::sink::FileSink::new(path)?),
            ))
        } else {
            re_log::debug!("Rerun disabled - call to save() ignored");
            Ok(Session::disabled())
        }
    }

    /// Returns whether or not logging is enabled, plus a [`RecordingInfo`].
    ///
    /// This can be used to then construct a [`Session`] manually using [`Session::new`].
    pub fn finalize(self) -> (bool, RecordingInfo) {
        let Self {
            application_id,
            is_official_example,
            enabled,
            default_enabled,
            recording_id,
        } = self;

        let enabled = enabled.unwrap_or_else(|| crate::decide_logging_enabled(default_enabled));
        let recording_id = recording_id.unwrap_or_else(RecordingId::random);

        let recording_info = RecordingInfo {
            application_id,
            recording_id,
            is_official_example,
            started: Time::now(),
            recording_source: RecordingSource::RustSdk {
                rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            },
        };

        (enabled, recording_info)
    }
}

// ----------------------------------------------------------------------------

/// The main way to do Rerun loggning.
///
/// You can construct a [`Session`] with [`SessionBuilder`] or [`Session::new`].
///
/// Cloning a [`Session`] is cheap (it's a shallow clone).
/// The clone will send its messages to the same sink as the prototype.
///
/// `Session` also implements `Send` and `Sync`.
#[must_use]
#[derive(Clone)]
pub struct Session {
    sink: Arc<dyn LogSink>,
    // TODO(emilk): add convenience `TimePoint` here so that users can
    // do things like `session.set_time_sequence("frame", frame_idx);`
}

#[test]
fn session_impl_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Session>();
}

impl Session {
    /// Construct a new session with a given [`RecordingInfo`] and [`LogSink`].
    ///
    /// You can create a [`RecordingInfo`] with [`crate::new_recording_info`];
    ///
    /// The [`RecordingInfo`] is immediately sent to the sink in the form of a
    /// [`re_log_types::BeginRecordingMsg`].
    ///
    /// You can find sinks in [`crate::sink`].
    ///
    /// See also: [`SessionBuilder`].
    pub fn new(recording_info: RecordingInfo, sink: Box<dyn LogSink>) -> Self {
        if sink.is_enabled() {
            re_log::debug!(
                "Beginning new recording with application_id {:?} and recording id {}",
                recording_info.application_id.0,
                recording_info.recording_id
            );

            sink.send(
                re_log_types::BeginRecordingMsg {
                    msg_id: re_log_types::MsgId::random(),
                    info: recording_info,
                }
                .into(),
            );
        }

        Self { sink: sink.into() }
    }

    /// Construct a new session with a disabled "dummy" sink that drops all logging messages.
    ///
    /// [`Self::is_enabled`] will return `false`.
    pub fn disabled() -> Self {
        Self {
            sink: crate::sink::disabled().into(),
        }
    }

    /// Buffer log messages in RAM.
    ///
    /// Retrieve them later with [`Self::drain_backlog`].
    pub fn buffered(recording_info: RecordingInfo) -> Self {
        Self::new(recording_info, Box::new(crate::sink::BufferedSink::new()))
    }

    /// Check if logging is enabled on this `Session`.
    ///
    /// If not, all logging calls will be ignored.
    pub fn is_enabled(&self) -> bool {
        self.sink.is_enabled()
    }

    /// Send a [`LogMsg`].
    pub fn send(&self, log_msg: LogMsg) {
        self.sink.send(log_msg);
    }

    /// Send a [`re_log_types::PathOp`].
    ///
    /// This is a convenience wrapper for [`Self::send`].
    pub fn send_path_op(
        &self,
        time_point: &re_log_types::TimePoint,
        path_op: re_log_types::PathOp,
    ) {
        self.send(LogMsg::EntityPathOpMsg(re_log_types::EntityPathOpMsg {
            msg_id: re_log_types::MsgId::random(),
            time_point: time_point.clone(),
            path_op,
        }));
    }

    /// Drain all buffered [`LogMsg`]es and return them.
    pub fn drain_backlog(&self) -> Vec<LogMsg> {
        self.sink.drain_backlog()
    }
}

impl AsRef<dyn LogSink> for Session {
    fn as_ref(&self) -> &dyn LogSink {
        self.sink.as_ref()
    }
}
