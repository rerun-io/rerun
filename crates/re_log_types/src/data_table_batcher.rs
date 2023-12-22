use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam::channel::{Receiver, Sender};

use re_types_core::SizeBytes as _;

use crate::{DataRow, DataTable, TableId};

// ---

/// Errors that can occur when creating/manipulating a [`DataTableBatcher`].
#[derive(thiserror::Error, Debug)]
pub enum DataTableBatcherError {
    /// Error when parsing configuration from environment.
    #[error("Failed to parse config: '{name}={value}': {err}")]
    ParseConfig {
        name: &'static str,
        value: String,
        err: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Error spawning one of the background threads.
    #[error("Failed to spawn background thread '{name}': {err}")]
    SpawnThread {
        name: &'static str,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type DataTableBatcherResult<T> = Result<T, DataTableBatcherError>;

// ---

/// Defines the different thresholds of the associated [`DataTableBatcher`].
///
/// See [`Self::default`] and [`Self::from_env`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableBatcherConfig {
    /// Duration of the periodic tick.
    //
    // NOTE: We use `std::time` directly because this library has to deal with `crossbeam` as well
    // as std threads, which both expect standard types anyway.
    //
    // TODO(cmc): Add support for burst debouncing.
    pub flush_tick: Duration,

    /// Flush if the accumulated payload has a size in bytes equal or greater than this.
    ///
    /// The resulting [`DataTable`] might be larger than `flush_num_bytes`!
    pub flush_num_bytes: u64,

    /// Flush if the accumulated payload has a number of rows equal or greater than this.
    pub flush_num_rows: u64,

    /// Size of the internal channel of commands.
    ///
    /// Unbounded if left unspecified.
    pub max_commands_in_flight: Option<u64>,

    /// Size of the internal channel of [`DataTable`]s.
    ///
    /// Unbounded if left unspecified.
    pub max_tables_in_flight: Option<u64>,

    /// Callback to be run when an Arrow [`Chunk`] goes out of scope.
    ///
    /// See [`crate::ArrowChunkReleaseCallback`] for more information.
    pub on_release: Option<crate::ArrowChunkReleaseCallback>,
}

impl Default for DataTableBatcherConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl DataTableBatcherConfig {
    /// Default configuration, applicable to most use cases.
    pub const DEFAULT: Self = Self {
        flush_tick: Duration::from_millis(8), // We want it fast enough for 60 Hz for real time camera feel
        flush_num_bytes: 1024 * 1024,         // 1 MiB
        flush_num_rows: u64::MAX,
        max_commands_in_flight: None,
        max_tables_in_flight: None,
        on_release: None,
    };

    /// Always flushes ASAP.
    pub const ALWAYS: Self = Self {
        flush_tick: Duration::MAX,
        flush_num_bytes: 0,
        flush_num_rows: 0,
        max_commands_in_flight: None,
        max_tables_in_flight: None,
        on_release: None,
    };

    /// Never flushes unless manually told to.
    pub const NEVER: Self = Self {
        flush_tick: Duration::MAX,
        flush_num_bytes: u64::MAX,
        flush_num_rows: u64::MAX,
        max_commands_in_flight: None,
        max_tables_in_flight: None,
        on_release: None,
    };

    /// Environment variable to configure [`Self::flush_tick`].
    pub const ENV_FLUSH_TICK: &'static str = "RERUN_FLUSH_TICK_SECS";

    /// Environment variable to configure [`Self::flush_num_bytes`].
    pub const ENV_FLUSH_NUM_BYTES: &'static str = "RERUN_FLUSH_NUM_BYTES";

    /// Environment variable to configure [`Self::flush_num_rows`].
    pub const ENV_FLUSH_NUM_ROWS: &'static str = "RERUN_FLUSH_NUM_ROWS";

    /// Creates a new `DataTableBatcherConfig` using the default values, optionally overridden
    /// through the environment.
    ///
    /// See [`Self::apply_env`].
    #[inline]
    pub fn from_env() -> DataTableBatcherResult<Self> {
        Self::default().apply_env()
    }

    /// Returns a copy of `self`, overriding existing fields with values from the environment if
    /// they are present.
    ///
    /// See [`Self::ENV_FLUSH_TICK`], [`Self::ENV_FLUSH_NUM_BYTES`], [`Self::ENV_FLUSH_NUM_BYTES`].
    pub fn apply_env(&self) -> DataTableBatcherResult<Self> {
        let mut new = self.clone();

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_TICK) {
            let flush_duration_secs: f64 =
                s.parse()
                    .map_err(|err| DataTableBatcherError::ParseConfig {
                        name: Self::ENV_FLUSH_TICK,
                        value: s.clone(),
                        err: Box::new(err),
                    })?;

            new.flush_tick = Duration::from_secs_f64(flush_duration_secs);
        }

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_NUM_BYTES) {
            new.flush_num_bytes = s
                .parse()
                .map_err(|err| DataTableBatcherError::ParseConfig {
                    name: Self::ENV_FLUSH_NUM_BYTES,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
        }

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_NUM_ROWS) {
            new.flush_num_rows = s
                .parse()
                .map_err(|err| DataTableBatcherError::ParseConfig {
                    name: Self::ENV_FLUSH_NUM_ROWS,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
        }

        Ok(new)
    }
}

#[test]
fn data_table_batcher_config() {
    // Detect breaking changes in our environment variables.
    std::env::set_var("RERUN_FLUSH_TICK_SECS", "0.3");
    std::env::set_var("RERUN_FLUSH_NUM_BYTES", "42");
    std::env::set_var("RERUN_FLUSH_NUM_ROWS", "666");

    let config = DataTableBatcherConfig::from_env().unwrap();

    let expected = DataTableBatcherConfig {
        flush_tick: Duration::from_millis(300),
        flush_num_bytes: 42,
        flush_num_rows: 666,
        ..Default::default()
    };

    assert_eq!(expected, config);
}

// ---

/// Implements an asynchronous batcher that coalesces [`DataRow`]s into [`DataTable`]s based upon
/// the thresholds defined in the associated [`DataTableBatcherConfig`].
///
/// ## Multithreading and ordering
///
/// [`DataTableBatcher`] can be cheaply clone and used freely across any number of threads.
///
/// Internally, all operations are linearized into a pipeline:
/// - All operations sent by a given thread will take effect in the same exact order as that
///   thread originally sent them in, from its point of view.
/// - There isn't any well defined global order across multiple threads.
///
/// This means that e.g. flushing the pipeline ([`Self::flush_blocking`]) guarantees that all
/// previous data sent by the calling thread has been batched and sent down the channel returned
/// by [`DataTableBatcher::tables`]; no more, no less.
///
/// ## Shutdown
///
/// The batcher can only be shutdown by dropping all instances of it, at which point it will
/// automatically take care of flushing any pending data that might remain in the pipeline.
///
/// Shutting down cannot ever block.
#[derive(Clone)]
pub struct DataTableBatcher {
    inner: Arc<DataTableBatcherInner>,
}

// NOTE: The receiving end of the command stream as well as the sending end of the table stream are
// owned solely by the batching thread.
struct DataTableBatcherInner {
    /// The one and only entrypoint into the pipeline: this is _never_ cloned nor publicly exposed,
    /// therefore the `Drop` implementation is guaranteed that no more data can come in while it's
    /// running.
    tx_cmds: Sender<Command>,
    // NOTE: Option so we can make shutdown non-blocking even with bounded channels.
    rx_tables: Option<Receiver<DataTable>>,
    cmds_to_tables_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for DataTableBatcherInner {
    fn drop(&mut self) {
        // Drop the receiving end of the table stream first and foremost, so that we don't block
        // even if the output channel is bounded and currently full.
        if let Some(rx_tables) = self.rx_tables.take() {
            if !rx_tables.is_empty() {
                re_log::warn!("Dropping data");
            }
        }

        // NOTE: The command channel is private, if we're here, nothing is currently capable of
        // sending data down the pipeline.
        self.tx_cmds.send(Command::Shutdown).ok();
        if let Some(handle) = self.cmds_to_tables_handle.take() {
            handle.join().ok();
        }
    }
}

enum Command {
    // TODO(cmc): support for appending full tables
    AppendRow(DataRow),
    Flush(Sender<()>),
    Shutdown,
}

impl Command {
    fn flush() -> (Self, Receiver<()>) {
        let (tx, rx) = crossbeam::channel::bounded(0); // oneshot
        (Self::Flush(tx), rx)
    }
}

impl DataTableBatcher {
    /// Creates a new [`DataTableBatcher`] using the passed in `config`.
    ///
    /// The returned object must be kept in scope: dropping it will trigger a clean shutdown of the
    /// batcher.
    #[must_use = "Batching threads will automatically shutdown when this object is dropped"]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(config: DataTableBatcherConfig) -> DataTableBatcherResult<Self> {
        let (tx_cmds, rx_cmd) = match config.max_commands_in_flight {
            Some(cap) => crossbeam::channel::bounded(cap as _),
            None => crossbeam::channel::unbounded(),
        };

        let (tx_table, rx_tables) = match config.max_tables_in_flight {
            Some(cap) => crossbeam::channel::bounded(cap as _),
            None => crossbeam::channel::unbounded(),
        };

        let cmds_to_tables_handle = {
            const NAME: &str = "DataTableBatcher::cmds_to_tables";
            std::thread::Builder::new()
                .name(NAME.into())
                .spawn({
                    let config = config.clone();
                    move || batching_thread(config, rx_cmd, tx_table)
                })
                .map_err(|err| DataTableBatcherError::SpawnThread {
                    name: NAME,
                    err: Box::new(err),
                })?
        };

        re_log::debug!(?config, "creating new table batcher");

        let inner = DataTableBatcherInner {
            tx_cmds,
            rx_tables: Some(rx_tables),
            cmds_to_tables_handle: Some(cmds_to_tables_handle),
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    // --- Send commands ---

    /// Pushes a [`DataRow`] down the batching pipeline.
    ///
    /// This will call [`DataRow::compute_all_size_bytes`] from the batching thread!
    ///
    /// See [`DataTableBatcher`] docs for ordering semantics and multithreading guarantees.
    #[inline]
    pub fn push_row(&self, row: DataRow) {
        self.inner.push_row(row);
    }

    /// Initiates a flush of the pipeline and returns immediately.
    ///
    /// This does **not** wait for the flush to propagate (see [`Self::flush_blocking`]).
    /// See [`DataTableBatcher`] docs for ordering semantics and multithreading guarantees.
    #[inline]
    pub fn flush_async(&self) {
        self.inner.flush_async();
    }

    /// Initiates a flush the batching pipeline and waits for it to propagate.
    ///
    /// See [`DataTableBatcher`] docs for ordering semantics and multithreading guarantees.
    #[inline]
    pub fn flush_blocking(&self) {
        self.inner.flush_blocking();
    }

    // --- Subscribe to tables ---

    /// Returns a _shared_ channel in which are sent the batched [`DataTable`]s.
    ///
    /// Shutting down the batcher will close this channel.
    ///
    /// See [`DataTableBatcher`] docs for ordering semantics and multithreading guarantees.
    pub fn tables(&self) -> Receiver<DataTable> {
        // NOTE: `rx_tables` is only ever taken when the batcher as a whole is dropped, at which
        // point it is impossible to call this method.
        self.inner.rx_tables.clone().unwrap()
    }
}

impl DataTableBatcherInner {
    fn push_row(&self, row: DataRow) {
        self.send_cmd(Command::AppendRow(row));
    }

    fn flush_async(&self) {
        let (flush_cmd, _) = Command::flush();
        self.send_cmd(flush_cmd);
    }

    fn flush_blocking(&self) {
        let (flush_cmd, oneshot) = Command::flush();
        self.send_cmd(flush_cmd);
        oneshot.recv().ok();
    }

    fn send_cmd(&self, cmd: Command) {
        // NOTE: Internal channels can never be closed outside of the `Drop` impl, this cannot
        // fail.
        self.tx_cmds.send(cmd).ok();
    }
}

#[allow(clippy::needless_pass_by_value)]
fn batching_thread(
    config: DataTableBatcherConfig,
    rx_cmd: Receiver<Command>,
    tx_table: Sender<DataTable>,
) {
    let rx_tick = crossbeam::channel::tick(config.flush_tick);

    struct Accumulator {
        latest: Instant,
        pending_rows: Vec<DataRow>,
        pending_num_rows: u64,
        pending_num_bytes: u64,
    }

    impl Accumulator {
        fn reset(&mut self) {
            self.latest = Instant::now();
            self.pending_rows.clear();
            self.pending_num_rows = 0;
            self.pending_num_bytes = 0;
        }
    }

    let mut acc = Accumulator {
        latest: Instant::now(),
        pending_rows: Default::default(),
        pending_num_rows: Default::default(),
        pending_num_bytes: Default::default(),
    };

    fn do_push_row(acc: &mut Accumulator, mut row: DataRow) {
        // TODO(#1760): now that we're re doing this here, it really is a massive waste not to send
        // it over the wire…
        row.compute_all_size_bytes();

        acc.pending_num_rows += 1;
        acc.pending_num_bytes += row.total_size_bytes();
        acc.pending_rows.push(row);
    }

    fn do_flush_all(acc: &mut Accumulator, tx_table: &Sender<DataTable>, reason: &str) {
        let rows = &mut acc.pending_rows;

        if rows.is_empty() {
            return;
        }

        re_log::trace!(reason, "flushing tables");

        let table = DataTable::from_rows(TableId::new(), rows.drain(..));
        // TODO(#1981): efficient table sorting here, following the same rules as the store's.
        // table.sort();

        // NOTE: This can only fail if all receivers have been dropped, which simply cannot happen
        // as long the batching thread is alive… which is where we currently are.
        tx_table.send(table).ok();

        acc.reset();
    }

    use crossbeam::select;
    loop {
        select! {
            recv(rx_cmd) -> cmd => {
                let Ok(cmd) = cmd else {
                    // All command senders are gone, which can only happen if the
                    // `DataTableBatcher` itself has been dropped.
                    break;
                };

            match cmd {
                Command::AppendRow(row) => {
                    do_push_row(&mut acc, row);
                    if acc.pending_num_rows >= config.flush_num_rows {
                        do_flush_all(&mut acc, &tx_table, "rows");
                    } else if acc.pending_num_bytes >= config.flush_num_bytes {
                        do_flush_all(&mut acc, &tx_table, "bytes");
                    }
                },
                Command::Flush(oneshot) => {
                    do_flush_all(&mut acc, &tx_table, "manual");
                    drop(oneshot); // signals the oneshot
                },
                Command::Shutdown => break,
            };
            },
            recv(rx_tick) -> _ => {
                do_flush_all(&mut acc, &tx_table, "duration");
            },
        };
    }

    drop(rx_cmd);
    do_flush_all(&mut acc, &tx_table, "shutdown");
    drop(tx_table);

    // NOTE: The receiving end of the command stream as well as the sending end of the table
    // stream are owned solely by this thread.
    // Past this point, all command writes and all table reads will return `ErrDisconnected`.
}
