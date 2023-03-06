use std::net::SocketAddr;

use re_log_types::{
    ApplicationId, BeginRecordingMsg, LogMsg, MsgId, PathOp, RecordingId, RecordingInfo,
    RecordingSource, Time, TimePoint,
};

use crate::file_writer::FileWriter;

#[cfg(feature = "web_viewer")]
use crate::remote_viewer_server::RemoteViewerServer;

/// This is the main object you need to create to use the Rerun SDK.
///
/// You should ideally create one session object and reuse it.
/// For convenience, there is a global [`Session`] object you can access with [`crate::global_session`].
pub struct Session {
    /// Is this session enabled?
    /// If not, all calls into it are ignored!
    enabled: bool,

    recording_source: RecordingSource,

    #[cfg(feature = "web_viewer")]
    tokio_rt: tokio::runtime::Runtime,

    sender: Sender,

    application_id: Option<ApplicationId>,
    recording_id: Option<RecordingId>,
    is_official_example: Option<bool>,

    has_sent_begin_recording_msg: bool,
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
        // official example detection
        let is_official_example = {
            // The sentinel file we use to identify the official examples directory.
            const SENTINEL_FILENAME: &str = ".rerun_examples";

            // TODO(cmc): Normally we should be collecting a full backtrace here in order to
            // support cases where the caller we're interested in isn't necessarily the direct one.
            // For now this'll do and avoids pulling `backtrace` and adding yet another feature
            // flag.
            let caller = core::panic::Location::caller();
            let mut path = std::path::PathBuf::from(caller.file());

            let mut is_official_example = false;
            // more than 4 layers would be really pushing it
            for _ in 0..4 {
                path.pop(); // first iteration is always a file path in our examples
                if path.join(SENTINEL_FILENAME).exists() {
                    is_official_example = true;
                }
            }

            is_official_example
        };

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

            recording_source: RecordingSource::RustSdk {
                rust_version: env!("CARGO_PKG_RUST_VERSION").into(),
            },

            #[cfg(feature = "web_viewer")]
            tokio_rt: tokio::runtime::Runtime::new().unwrap(),

            sender: Default::default(),
            application_id: None,
            recording_id: None,
            is_official_example: None,
            has_sent_begin_recording_msg: false,
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

    /// Set the [`ApplicationId`] to use for the following stream of log messages.
    ///
    /// This should be called once before anything else.
    /// If you don't call this, the resulting application id will be [`ApplicationId::unknown`].
    ///
    /// Note that many recordings can share the same [`ApplicationId`], but
    /// they all have unique [`RecordingId`]s.
    pub fn set_application_id(&mut self, application_id: ApplicationId, is_official_example: bool) {
        if self.application_id.as_ref() != Some(&application_id) {
            self.application_id = Some(application_id);
            self.is_official_example = Some(is_official_example);
            self.has_sent_begin_recording_msg = false;
        }
    }

    /// The current [`RecordingId`], if set.
    pub fn recording_id(&self) -> Option<RecordingId> {
        self.recording_id
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
        if self.recording_id != Some(recording_id) {
            self.recording_id = Some(recording_id);
            self.has_sent_begin_recording_msg = false;
        }
    }

    /// Set where the recording is coming from.
    /// The default is [`RecordingSource::RustSdk`].
    pub fn set_recording_source(&mut self, recording_source: RecordingSource) {
        self.recording_source = recording_source;
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

        let backlog = self.drain_log_messages_buffer();

        match &mut self.sender {
            Sender::Remote(remote) => {
                remote.set_addr(addr);
            }

            #[cfg(feature = "re_viewer")]
            Sender::NativeViewer(_) => {
                re_log::error!("Cannot connect from within a spawn() call");
            }

            _ => {
                re_log::debug!("Connecting to remote…");
                let mut client = re_sdk_comms::Client::new(addr);
                for msg in backlog {
                    client.send(msg);
                }
                self.sender = Sender::Remote(client);
            }
        }
    }

    /// Serve log-data over WebSockets and serve a Rerun web viewer over HTTP.
    ///
    /// If the `open_browser` argument is `true`, your default browser
    /// will be opened with a connected web-viewer.
    ///
    /// If not, you can connect to this server using the `rerun` binary (`cargo install rerun`).
    ///
    /// NOTE: you can not connect one `Session` to another.
    ///
    /// This function returns immediately.
    #[cfg(feature = "web_viewer")]
    pub fn serve(&mut self, open_browser: bool) {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to serve() ignored");
            return;
        }

        self.sender = Sender::WebViewer(RemoteViewerServer::new(&self.tokio_rt, open_browser));
    }

    /// Disconnects any TCP connection, shuts down any server, and closes any file.
    pub fn disconnect(&mut self) {
        if !matches!(&self.sender, &Sender::Buffered(_)) {
            re_log::debug!("Switching to buffered.");
            self.sender = Sender::Buffered(Default::default());
        }
    }

    /// Are we streaming log messages over TCP?
    ///
    /// Returns true after a call to [`Self::connect`].
    ///
    /// Returns `false` if we are serving the messages to a web viewer,
    /// or if we are buffering the messages (to save them to file later).
    ///
    /// This can return true even before the connection is yet to be established.
    pub fn is_streaming_over_tcp(&self) -> bool {
        matches!(&self.sender, &Sender::Remote(_))
    }

    /// Wait until all logged data have been sent to the remove server (if any).
    pub fn flush(&mut self) {
        if let Sender::Remote(sender) = &mut self.sender {
            sender.flush();
        }
    }

    /// If the tcp session is disconnected, allow it to quit early and drop unsent messages
    pub fn drop_msgs_if_disconnected(&mut self) {
        if let Sender::Remote(sender) = &mut self.sender {
            sender.drop_if_disconnected();
        }
    }

    /// Drain all buffered [`LogMsg`]es and return them.
    pub fn drain_log_messages_buffer(&mut self) -> Vec<LogMsg> {
        if let Sender::Buffered(log_messages) = &mut self.sender {
            std::mem::take(log_messages)
        } else {
            vec![]
        }
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
            if let Some(recording_id) = self.recording_id {
                let application_id = self
                    .application_id
                    .clone()
                    .unwrap_or_else(ApplicationId::unknown);

                re_log::debug!(
                    "Beginning new recording with application_id {:?} and recording id {}",
                    application_id.0,
                    recording_id
                );

                self.sender.send(
                    BeginRecordingMsg {
                        msg_id: MsgId::random(),
                        info: RecordingInfo {
                            application_id,
                            recording_id,
                            is_official_example: self.is_official_example.unwrap_or_default(),
                            started: Time::now(),
                            recording_source: self.recording_source.clone(),
                        },
                    }
                    .into(),
                );
                self.has_sent_begin_recording_msg = true;
            }
        }

        self.sender.send(log_msg);
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

        let file_writer = FileWriter::new(path)?;
        let backlog = self.drain_log_messages_buffer();
        for log_msg in backlog {
            file_writer.write(log_msg);
        }
        self.sender = Sender::File(file_writer);
        Ok(())
    }
}

#[cfg(feature = "native_viewer")]
impl Session {
    fn app_env(&self) -> re_viewer::AppEnvironment {
        match &self.recording_source {
            RecordingSource::PythonSdk(python_version) => {
                re_viewer::AppEnvironment::PythonSdk(python_version.clone())
            }
            RecordingSource::RustSdk { rust_version } => re_viewer::AppEnvironment::RustSdk {
                rust_version: rust_version.clone(),
            },
            RecordingSource::Unknown | RecordingSource::Other(_) => {
                re_viewer::AppEnvironment::RustSdk {
                    rust_version: "unknown".into(),
                }
            }
        }
    }

    /// Drains all pending log messages and starts a Rerun viewer to visualize everything that has
    /// been logged so far.
    pub fn show(&mut self) -> re_viewer::external::eframe::Result<()> {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to show() ignored");
            return Ok(());
        }

        let log_messages = self.drain_log_messages_buffer();
        let startup_options = re_viewer::StartupOptions::default();
        re_viewer::run_native_viewer_with_messages(
            re_build_info::build_info!(),
            self.app_env(),
            startup_options,
            log_messages,
        )
    }

    /// Starts a Rerun viewer on the current thread and migrates the given callback, along with
    /// the active `Session`, to a newly spawned thread where the callback will run until
    /// completion.
    ///
    /// All messages logged from the passed-in callback will be streamed to the viewer in
    /// real-time.
    ///
    /// This method will not return as long as the viewer runs.
    ///
    /// ⚠️  This function must be called from the main thread since some platforms require that
    /// their UI runs on the main thread! ⚠️
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn<F, T>(mut self, run: F) -> re_viewer::external::eframe::Result<()>
    where
        F: FnOnce(Session) -> T + Send + 'static,
        T: Send + 'static,
    {
        if !self.enabled {
            re_log::debug!("Rerun disabled - call to spawn() ignored");
            return Ok(());
        }

        let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::Sdk);

        for msg in self.drain_log_messages_buffer() {
            tx.send(msg).ok();
        }

        self.sender = Sender::NativeViewer(tx);
        let app_env = self.app_env();

        // NOTE: Forget the handle on purpose, leave that thread be.
        std::thread::Builder::new()
            .name("spawned".into())
            .spawn(move || run(self))
            .expect("Failed to spawn thread");

        // NOTE: Some platforms still mandate that the UI must run on the main thread, so make sure
        // to spawn the viewer in place and migrate the user callback to a new thread.
        re_viewer::run_native_app(Box::new(move |cc, re_ui| {
            // TODO(cmc): it'd be nice to centralize all the UI wake up logic somewhere.
            let rx = re_viewer::wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
            let startup_options = re_viewer::StartupOptions::default();
            Box::new(re_viewer::App::from_receiver(
                re_build_info::build_info!(),
                &app_env,
                startup_options,
                re_ui,
                cc.storage,
                rx,
            ))
        }))
    }
}

enum Sender {
    Buffered(Vec<LogMsg>),

    File(FileWriter),

    Remote(re_sdk_comms::Client),

    #[cfg(feature = "native_viewer")]
    NativeViewer(re_smart_channel::Sender<LogMsg>),

    /// Serve it to the web viewer over WebSockets
    #[cfg(feature = "web_viewer")]
    WebViewer(RemoteViewerServer),
}

impl Default for Sender {
    fn default() -> Self {
        Sender::Buffered(vec![])
    }
}

impl Sender {
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
}
