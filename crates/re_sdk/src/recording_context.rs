use crossbeam::channel::{Receiver, SendError, Sender};
use re_log_types::{
    ApplicationId, DataRow, DataTable, DataTableBatcher, DataTableBatcherConfig, LogMsg,
    RecordingId, RecordingInfo, RecordingSource, Time,
};

use crate::sink::{LogSink, MemorySinkStorage};

// ---

/// Errors that can occur when creating/manipulating a [`DataTableBatcher`].
#[derive(thiserror::Error, Debug)]
pub enum RecordingContextError {
    /// Error when changing state of the recording.
    #[error("Illegal state change: cannot go from '{from}' to '{to}'")]
    IllegalStateChange {
        from: &'static str,
        to: &'static str,
    },

    /// Error spawning one of the background threads.
    #[error("Failed to spawn background thread '{name}': {err}")]
    SpawnThread {
        name: &'static str,
        err: Box<dyn std::error::Error>,
    },

    /// The recording is closed.
    #[error("The recording is closed")]
    Closed { data: Box<dyn std::any::Any> },
}

pub type RecordingContextResult<T> = Result<T, RecordingContextError>;

// ---

/// Construct a [`RecordingContext`].
///
/// ``` no_run
/// # use re_sdk::RecordingContextBuilder;
/// let rec_ctx = RecordingContextBuilder::new("my_app").save("my_recording.rrd")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub struct RecordingContextBuilder {
    application_id: ApplicationId,
    recording_id: Option<RecordingId>,
    recording_source: Option<RecordingSource>,

    default_enabled: bool,
    enabled: Option<bool>,

    is_official_example: bool,
}

impl RecordingContextBuilder {
    /// Create a new [`RecordingContextBuilder`] with the given [`ApplicationId`].
    ///
    /// The [`ApplicationId`] is usually the name of your app.
    ///
    /// ```no_run
    /// # use re_sdk::RecordingContextBuilder;
    /// let rec_ctx = RecordingContextBuilder::new("my_app").save("my_recording.rrd")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    //
    // NOTE: track_caller so that we can see if we are being called from an official example.
    #[track_caller]
    pub fn new(application_id: impl Into<ApplicationId>) -> Self {
        let application_id = application_id.into();
        let is_official_example = crate::called_from_official_rust_example();

        Self {
            application_id,
            recording_id: None,
            recording_source: None,

            default_enabled: true,
            enabled: None,

            is_official_example,
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

    /// Set the [`RecordingId`] for this context.
    ///
    /// If you're logging from multiple processes and want all the messages to end up as the same
    /// recording, you must make sure they all set the same [`RecordingId`] using this function.
    ///
    /// Note that many recordings can share the same [`ApplicationId`], but they all have
    /// unique [`RecordingId`]s.
    ///
    /// The default is to use a random [`RecordingId`].
    pub fn recording_id(mut self, recording_id: RecordingId) -> Self {
        self.recording_id = Some(recording_id);
        self
    }

    pub fn recording_source(mut self, recording_source: RecordingSource) -> Self {
        self.recording_source = Some(recording_source);
        self
    }

    pub fn is_official_example(mut self, is_official_example: bool) -> Self {
        self.is_official_example = is_official_example;
        self
    }

    /// Creates a new [`RecordingContext`] that starts in a buffering state (RAM).
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec_ctx = re_sdk::RecordingContextBuilder::new("my_app").buffered();
    /// ```
    pub fn buffered(self) -> RecordingContext {
        let (rerun_enabled, recording_info) = self.finalize();
        RecordingContext::new(
            recording_info,
            if rerun_enabled {
                Some(Box::new(crate::log_sink::BufferedSink::new()))
            } else {
                re_log::debug!("Rerun disabled - call to buffered() ignored");
                None
            },
        )
    }

    /// Creates a new [`RecordingContext`] that is pre-configured to stream the data through to a
    /// [`MemorySink`].
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let (rec_ctx, storage) = re_sdk::RecordingContextBuilder::new("my_app").memory();
    /// ```
    pub fn memory_recording(self) -> (RecordingContext, crate::log_sink::MemorySinkStorage) {
        let sink = crate::log_sink::MemorySink::default();
        let storage = sink.buffer();

        let (rerun_enabled, recording_info) = self.finalize();
        (
            RecordingContext::new(
                recording_info,
                if rerun_enabled {
                    Some(Box::new(sink))
                } else {
                    re_log::debug!("Rerun disabled - call to memory() ignored");
                    None
                },
            ),
            storage,
        )
    }

    /// Creates a new [`RecordingContext`] that is pre-configured to stream the data through to a
    /// remote Rerun instance.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec_ctx = re_sdk::RecordingContextBuilder::new("my_app")
    ///     .connect(re_sdk::default_server_addr());
    /// ```
    pub fn connect(self, addr: std::net::SocketAddr) -> RecordingContext {
        let (rerun_enabled, recording_info) = self.finalize();
        RecordingContext::new(
            recording_info,
            if rerun_enabled {
                Some(Box::new(crate::log_sink::TcpSink::new(addr)))
            } else {
                re_log::debug!("Rerun disabled - call to connect() ignored");
                None
            },
        )
    }

    /// Creates a new [`RecordingContext`] that is pre-configured to stream the data through to an
    /// RRD file on disk.
    ///
    /// ## Example
    ///
    /// ```no_run
    /// let rec_ctx = re_sdk::RecordingContextBuilder::new("my_app").save("my_recording.rrd")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(
        self,
        path: impl Into<std::path::PathBuf>,
    ) -> Result<RecordingContext, crate::sink::FileSinkError> {
        let (rerun_enabled, recording_info) = self.finalize();
        Ok(RecordingContext::new(
            recording_info,
            if rerun_enabled {
                Some(Box::new(crate::sink::FileSink::new(path)?))
            } else {
                re_log::debug!("Rerun disabled - call to save() ignored");
                None
            },
        ))
    }

    /// Returns whether or not logging is enabled, plus a [`RecordingInfo`].
    ///
    /// This can be used to then construct a [`RecordingContext`] manually using
    /// [`RecordingContext::new`].
    pub fn finalize(self) -> (bool, RecordingInfo) {
        let Self {
            application_id,
            recording_id,
            recording_source,
            default_enabled,
            enabled,
            is_official_example,
        } = self;

        let enabled = enabled.unwrap_or_else(|| crate::decide_logging_enabled(default_enabled));
        let recording_id = recording_id.unwrap_or_else(RecordingId::random);
        let recording_source = recording_source.unwrap_or_else(|| RecordingSource::RustSdk {
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
        });

        let recording_info = RecordingInfo {
            application_id,
            recording_id,
            is_official_example,
            started: Time::now(),
            recording_source,
        };

        (enabled, recording_info)
    }
}

// ----------------------------------------------------------------------------

// TODO: when/how/should this shutdown?

// TODO: RwLock the entire thing so there's some kind of global order?

// TODO: this makes sure everything gets flushed when dropped; caller can call a special method to
// discard everything if they want to not block on shutdown.

// TODO: handle multithreading on behalf on the people

// TODO: gotta document what happens when doing weird multithreading stuff

// TODO: this needs a flush method then?

// TODO: no matter what, this needs a way to force shutdown

// TODO: closed state?

// TODO: the fact that this is clone probably mean we're heading for a race disaster with the
// batcher
/// The main way to do Rerun loggning.
///
/// You can construct a [`RecordingContext`] with [`RecordingContextBuilder`] or [`RecordingContext::new`].
///
/// Cloning a [`RecordingContext`] is cheap (it's a shallow clone).
/// The clone will send its messages to the same sink as the prototype.
///
/// `RecordingContext` also implements `Send` and `Sync`.
//
// TODO: doc:
// - clone
// - multithreading
// - shutdown
// - flush
// - drop
// - cheap to clone
#[must_use]
pub struct RecordingContext {
    enabled: bool,
    info: RecordingInfo,

    cmds_tx: Sender<Command>,
    // msgs_tx: Sender<LogMsg>,
    // sinks_tx: Sender<Box<dyn LogSink>>,
    batcher: DataTableBatcher,
    batcher_to_sink_handle: Option<std::thread::JoinHandle<()>>,
    //
    // TODO(emilk): add convenience `TimePoint` here so that users can
    // do things like `session.set_time_sequence("frame", frame_idx);`
}

// #[test]
// fn recording_context_impl_send_sync() {
//     fn assert_send_sync<T: Send + Sync>() {}
//     assert_send_sync::<RecordingContext>();
// }

enum Command {
    RecordMsg(LogMsg),
    SwapSink(Box<dyn LogSink>),
    Flush(Sender<()>),
    PopPendingTables,
}

impl Command {
    fn flush() -> (Self, Receiver<()>) {
        let (tx, rx) = crossbeam::channel::bounded(0); // oneshot
        (Self::Flush(tx), rx)
    }
}

impl RecordingContext {
    // TODO
    /// Construct a new `RecordingContext` with a given [`RecordingInfo`] and [`LogSink`].
    ///
    /// You can create a [`RecordingInfo`] with [`crate::new_recording_info`];
    ///
    /// The [`RecordingInfo`] is immediately sent to the sink in the form of a
    /// [`re_log_types::BeginRecordingMsg`].
    ///
    /// You can find sinks in [`crate::sink`].
    ///
    /// See also: [`RecordingContextBuilder`].
    #[must_use = "Recording will get closed automatically when this object is dropped"]
    pub fn new(info: RecordingInfo, mut sink: Option<Box<dyn LogSink>>) -> Self {
        let batcher = {
            let config = match DataTableBatcherConfig::from_env() {
                Ok(config) => config,
                Err(err) => {
                    re_log::error!(
                        %err,
                        "failed to load batching configuration, reverting to defaults"
                    );
                    DataTableBatcherConfig::default()
                }
            };
            DataTableBatcher::new(config).unwrap() // TODO
        };

        let enabled = sink.is_some();

        // TODO: BeginRecordingMsg is a misnomer
        if let Some(sink) = sink.as_mut() {
            re_log::debug!(
                app_id = %info.application_id,
                rec_id = %info.recording_id,
                "setting recording info",
            );
            sink.send(
                re_log_types::BeginRecordingMsg {
                    row_id: re_log_types::RowId::random(),
                    info: info.clone(),
                }
                .into(),
            );
        }

        let (cmds_tx, cmds_rx) = crossbeam::channel::unbounded();
        // let (msgs_tx, msgs_rx) = crossbeam::channel::unbounded();
        // let (sinks_tx, sinks_rx) = crossbeam::channel::unbounded();

        // TODO: do we want to not even spawn the thread?
        let batcher_to_sink_handle = {
            const NAME: &str = "RecordingContext::batcher_to_sink";
            std::thread::Builder::new()
                .name(NAME.into())
                .spawn({
                    let info = info.clone();
                    let batcher = batcher.clone();
                    let cmds_tx = cmds_tx.clone();
                    move || forwarding_thread(info, sink, cmds_tx, cmds_rx, batcher.tables())
                })
                .unwrap()
        }; // TODO

        Self {
            enabled,
            info,
            cmds_tx,
            batcher,
            batcher_to_sink_handle: Some(batcher_to_sink_handle),
        }
    }

    // TODO
    /// Construct a new session with a disabled "dummy" sink that drops all logging messages.
    ///
    /// [`Self::is_enabled`] will return `false`.
    pub fn disabled() -> Self {
        Self::new(
            RecordingInfo {
                application_id: ApplicationId::unknown(),
                recording_id: Default::default(),
                is_official_example: crate::called_from_official_rust_example(),
                started: Time::now(),
                recording_source: RecordingSource::RustSdk {
                    rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                    llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
                },
            },
            None,
        )
    }
}

// TODO: when are we supposed to leave here btw
#[allow(clippy::needless_pass_by_value)]
fn forwarding_thread(
    info: RecordingInfo,
    mut sink: Option<Box<dyn LogSink>>,
    cmds_tx: Sender<Command>,
    cmds_rx: Receiver<Command>,
    tables: Receiver<(RecordingId, DataTable)>,
) {
    fn handle_cmd(info: &RecordingInfo, cmd: Command, sink: &mut Option<Box<dyn LogSink>>) {
        match cmd {
            Command::RecordMsg(msg) => {
                unreachable!();
                if let Some(sink) = sink.as_mut() {
                    eprintln!("sending table down sink");
                    sink.send(msg);
                }
            }
            Command::SwapSink(new_sink) => {
                let mut backlog = Vec::new();
                if let Some(sink) = sink.as_mut() {
                    // TODO
                    // Capture the backlog (should only apply if this was a `BufferedSink`)
                    backlog = sink.drain_backlog();

                    // Before changing the sink, we set drop_if_disconnected and
                    // flush. This ensures that any messages that are currently
                    // buffered will be sent.
                    sink.drop_msgs_if_disconnected(); // TODO: that one scares me a little
                    sink.flush();
                }

                // TODO: BeginRecordingMsg is a misnomer
                {
                    re_log::debug!(
                        app_id = %info.application_id,
                        rec_id = %info.recording_id,
                        "setting recording info",
                    );
                    new_sink.send(
                        re_log_types::BeginRecordingMsg {
                            row_id: re_log_types::RowId::random(),
                            info: info.clone(),
                        }
                        .into(),
                    );
                    new_sink.send_all(backlog);
                }

                *sink = Some(new_sink);
            }
            Command::Flush(oneshot) => {
                if let Some(sink) = sink.as_mut() {
                    eprintln!("asking sink to flush");
                    // TODO: hmmm have to think real hard about that one..
                    sink.drop_msgs_if_disconnected(); // TODO: that one scares me a little
                    sink.flush();
                    eprintln!("asking sink to flush - done");
                }
                drop(oneshot); // signals the oneshot
            }
            Command::PopPendingTables => {} // TODO: explain
        }
    }

    use crossbeam::select;
    loop {
        // NOTE: Always handle tables first, this is what makes `RecordingContext::flush_blocking`
        // possible.
        while let Ok((rid, table)) = tables.try_recv() {
            // TODO: err
            let msg = table.to_arrow_msg().unwrap();
            if let Some(sink) = sink.as_mut() {
                eprintln!("sending table down sink");
                sink.send(LogMsg::ArrowMsg(rid, msg));
            }
            // sink.send(LogMsg::ArrowMsg(rid, msg));
            // cmds_tx
            //     .send(Command::RecordMsg(LogMsg::ArrowMsg(rid, msg)))
            //     .unwrap(); // TODO
        }

        select! {
            recv(tables) -> res => {
                let (rid, table) = res.unwrap(); // TODO
                let msg = table.to_arrow_msg().unwrap();
                if let Some(sink) = sink.as_mut() {
                    eprintln!("sending table down sink");
                    sink.send(LogMsg::ArrowMsg(rid, msg));
                }
                // cmds_tx
                //     .send(Command::RecordMsg(LogMsg::ArrowMsg(rid, msg)))
                //     .unwrap(); // TODO
            }
            recv(cmds_rx) -> res => {
                // let msg = res.unwrap(); // TODO
                if let Ok(cmd) = res {
                    handle_cmd(&info, cmd, &mut sink);
                }
            }
        }
    }
}

impl RecordingContext {
    /// Check if logging is enabled on this `RecordingContext`.
    ///
    /// If not, all logging calls will be ignored.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // TODO: if we make this atomic, you can effectively make RecordingContext natively multithread
    #[inline]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// The current [`RecordingId`].
    #[inline]
    pub fn recording_id(&self) -> RecordingId {
        self.info.recording_id
    }
}

impl RecordingContext {
    // TODO: What are the semantics of recording? at most once? at least once?

    // TODO: these record methods should absolutely give you an error if whatever you're recording
    // is about to be ignored.

    // TODO: do we even need those in this brand new world?

    // TODO: can only fail if already disconnected
    /// Record a [`LogMsg`].
    #[inline]
    pub fn record_msg(&self, msg: LogMsg) -> Result<(), SendError<LogMsg>> {
        if !self.is_enabled() {
            re_log::debug!("Recording disabled - log call ignored");
        }

        self.cmds_tx.send(Command::RecordMsg(msg)).map_err(|err| {
            SendError(match err.0 {
                Command::RecordMsg(msg) => msg,
                _ => unreachable!(),
            })
        })
    }

    // TODO: can only fail if already disconnected
    /// Send a [`re_log_types::PathOp`].
    ///
    /// This is a convenience wrapper for [`Self::send`].
    #[inline]
    pub fn record_path_op(
        &self,
        timepoint: re_log_types::TimePoint,
        path_op: re_log_types::PathOp,
    ) -> Result<(), SendError<LogMsg>> {
        self.record_msg(LogMsg::EntityPathOpMsg(
            self.recording_id(),
            re_log_types::EntityPathOpMsg {
                row_id: re_log_types::RowId::random(),
                time_point: timepoint,
                path_op,
            },
        ))
    }

    // TODO: err handling
    /// Record a single [`DataRow`].
    #[inline]
    pub fn record_row(&self, row: DataRow) -> Result<(), SendError<DataRow>> {
        if !self.is_enabled() {
            re_log::debug!("Recording disabled - log call ignored");
        }

        self.batcher
            .append_row(self.recording_id(), row)
            .map_err(|err| SendError(err.0 .1))
    }

    // TODO: guarantees? none if you don't flush first!! don't expose it?
    pub fn set_sink(&self, sink: Box<dyn LogSink>) -> Result<(), SendError<Box<dyn LogSink>>> {
        // Flush everything down the current active sink first...
        eprintln!("flushing everything");
        // if self.flush_blocking().is_err() {
        //     // NOTE: no `map_err` because we have to trick the borrowck
        //     return Err(SendError(sink));
        // }

        // 1. Flush the batcher down the table channel
        self.batcher.flush_blocking().unwrap();

        eprintln!("flushing everything - done");

        // 2. Receive pending tables from the batcher's channel
        self.cmds_tx.send(Command::PopPendingTables).unwrap();

        // 3. Swap the sink, which will internally make sure to flush it first
        self.cmds_tx.send(Command::SwapSink(sink)).map_err(|err| {
            SendError(match err.0 {
                Command::SwapSink(sink) => sink,
                _ => unreachable!(),
            })
        })?;

        // 4. Before we give control back to the caller, we need to make sure that the swap has
        //    taken place: we don't want the user to send data to the old sink!
        let (cmd, oneshot) = Command::flush();
        self.cmds_tx.send(cmd).unwrap();
        oneshot.recv().ok();

        Ok(())
    }

    // TODO
    pub fn flush_blocking(&self) -> Result<(), SendError<()>> {
        self.batcher.flush_blocking()?;

        // TODO: think more about this
        // NOTE: At this point we're guaranteed that the latest tables are available in the tables
        // channel.
        // Since tables are always handled _before_ commands, the cross-channel flush is properly
        // ordered.

        self.cmds_tx
            .send(Command::PopPendingTables)
            .map_err(|_err| SendError(()))?;

        let (cmd, oneshot) = Command::flush();
        self.cmds_tx.send(cmd).map_err(|_err| SendError(()))?;
        oneshot.recv().ok();

        Ok(())
    }
}

// TODO: set_sink helpers basically
// TODO: still not sure what to do with these send errors though
impl RecordingContext {
    // TODO
    // - really really have to explain the expected ordering here holy...
    // TODO: as a user, you want to be guaranteed that all following log calls will go to this sink
    pub fn connect(&self, addr: std::net::SocketAddr) -> Result<(), SendError<()>> {
        self.set_sink(Box::new(crate::log_sink::TcpSink::new(addr)))
            .map_err(|_err| SendError(()))
    }

    pub fn memory_recording(&self) -> Result<MemorySinkStorage, SendError<()>> {
        let sink = crate::sink::MemorySink::default();
        let buffer = sink.buffer();
        self.set_sink(Box::new(sink))
            .map(|_| buffer)
            .map_err(|_err| SendError(()))
    }

    pub fn save(&self, path: impl Into<std::path::PathBuf>) -> Result<(), SendError<()>> {
        let sink = crate::sink::FileSink::new(path).unwrap(); // TODO
        self.set_sink(Box::new(sink)).map_err(|_err| SendError(()))
    }

    // TODO: disconnect
    pub fn disconnect(&self) -> Result<(), SendError<()>> {
        self.set_sink(Box::new(crate::sink::BufferedSink::new()))
            .map_err(|_err| SendError(()))
    }

    // TODO:
    // - connect
    // - serve
    // - memory
    // - buffered (disconnect?)
    // - save
}

// TODO: test this hell, especially the flushing
