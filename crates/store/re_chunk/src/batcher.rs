use std::{
    hash::{Hash as _, Hasher},
    sync::Arc,
    time::{Duration, Instant},
};

use arrow2::array::{Array as Arrow2Array, PrimitiveArray as Arrow2PrimitiveArray};
use crossbeam::channel::{Receiver, Sender};
use nohash_hasher::IntMap;

use re_byte_size::SizeBytes as _;
use re_log_types::{EntityPath, ResolvedTimeRange, TimeInt, TimePoint, Timeline};
use re_types_core::ComponentDescriptor;

use crate::{chunk::ChunkComponents, Chunk, ChunkId, ChunkResult, RowId, TimeColumn};

// ---

/// Errors that can occur when creating/manipulating a [`ChunkBatcher`].
#[derive(thiserror::Error, Debug)]
pub enum ChunkBatcherError {
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

pub type ChunkBatcherResult<T> = Result<T, ChunkBatcherError>;

/// Callbacks you can install on the [`ChunkBatcher`].
#[derive(Clone, Default)]
pub struct BatcherHooks {
    /// Called when a new row arrives.
    ///
    /// The callback is given the slice of all rows not yet batched,
    /// including the new one.
    ///
    /// Used for testing.
    #[allow(clippy::type_complexity)]
    pub on_insert: Option<Arc<dyn Fn(&[PendingRow]) + Send + Sync>>,

    /// Callback to be run when an Arrow Chunk goes out of scope.
    ///
    /// See [`re_log_types::ArrowChunkReleaseCallback`] for more information.
    //
    // TODO(#6412): probably don't need this anymore.
    pub on_release: Option<re_log_types::ArrowChunkReleaseCallback>,
}

impl BatcherHooks {
    pub const NONE: Self = Self {
        on_insert: None,
        on_release: None,
    };
}

impl PartialEq for BatcherHooks {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            on_insert,
            on_release,
        } = self;

        let on_insert_eq = match (on_insert, &other.on_insert) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };

        on_insert_eq && on_release == &other.on_release
    }
}

impl std::fmt::Debug for BatcherHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            on_insert,
            on_release,
        } = self;
        f.debug_struct("BatcherHooks")
            .field("on_insert", &on_insert.as_ref().map(|_| "…"))
            .field("on_release", &on_release)
            .finish()
    }
}

// ---

/// Defines the different thresholds of the associated [`ChunkBatcher`].
///
/// See [`Self::default`] and [`Self::from_env`].
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkBatcherConfig {
    /// Duration of the periodic tick.
    //
    // NOTE: We use `std::time` directly because this library has to deal with `crossbeam` as well
    // as std threads, which both expect standard types anyway.
    //
    // TODO(cmc): Add support for burst debouncing.
    pub flush_tick: Duration,

    /// Flush if the accumulated payload has a size in bytes equal or greater than this.
    ///
    /// The resulting [`Chunk`] might be larger than `flush_num_bytes`!
    pub flush_num_bytes: u64,

    /// Flush if the accumulated payload has a number of rows equal or greater than this.
    pub flush_num_rows: u64,

    /// Split a chunk if it contains >= rows than this threshold and one or more of its timelines are
    /// unsorted.
    pub chunk_max_rows_if_unsorted: u64,

    /// Size of the internal channel of commands.
    ///
    /// Unbounded if left unspecified.
    pub max_commands_in_flight: Option<u64>,

    /// Size of the internal channel of [`Chunk`]s.
    ///
    /// Unbounded if left unspecified.
    pub max_chunks_in_flight: Option<u64>,

    /// Callbacks you can install on the [`ChunkBatcher`].
    pub hooks: BatcherHooks,
}

impl Default for ChunkBatcherConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ChunkBatcherConfig {
    /// Default configuration, applicable to most use cases.
    pub const DEFAULT: Self = Self {
        flush_tick: Duration::from_millis(8), // We want it fast enough for 60 Hz for real time camera feel
        flush_num_bytes: 1024 * 1024,         // 1 MiB
        flush_num_rows: u64::MAX,
        chunk_max_rows_if_unsorted: 256,
        max_commands_in_flight: None,
        max_chunks_in_flight: None,
        hooks: BatcherHooks::NONE,
    };

    /// Always flushes ASAP.
    pub const ALWAYS: Self = Self {
        flush_tick: Duration::MAX,
        flush_num_bytes: 0,
        flush_num_rows: 0,
        chunk_max_rows_if_unsorted: 256,
        max_commands_in_flight: None,
        max_chunks_in_flight: None,
        hooks: BatcherHooks::NONE,
    };

    /// Never flushes unless manually told to (or hitting one the builtin invariants).
    pub const NEVER: Self = Self {
        flush_tick: Duration::MAX,
        flush_num_bytes: u64::MAX,
        flush_num_rows: u64::MAX,
        chunk_max_rows_if_unsorted: 256,
        max_commands_in_flight: None,
        max_chunks_in_flight: None,
        hooks: BatcherHooks::NONE,
    };

    /// Environment variable to configure [`Self::flush_tick`].
    pub const ENV_FLUSH_TICK: &'static str = "RERUN_FLUSH_TICK_SECS";

    /// Environment variable to configure [`Self::flush_num_bytes`].
    pub const ENV_FLUSH_NUM_BYTES: &'static str = "RERUN_FLUSH_NUM_BYTES";

    /// Environment variable to configure [`Self::flush_num_rows`].
    pub const ENV_FLUSH_NUM_ROWS: &'static str = "RERUN_FLUSH_NUM_ROWS";

    /// Environment variable to configure [`Self::chunk_max_rows_if_unsorted`].
    //
    // NOTE: Shared with the same env-var on the store side, for consistency.
    pub const ENV_CHUNK_MAX_ROWS_IF_UNSORTED: &'static str = "RERUN_CHUNK_MAX_ROWS_IF_UNSORTED";

    /// Environment variable to configure [`Self::chunk_max_rows_if_unsorted`].
    #[deprecated(note = "use `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED` instead")]
    const ENV_MAX_CHUNK_ROWS_IF_UNSORTED: &'static str = "RERUN_MAX_CHUNK_ROWS_IF_UNSORTED";

    /// Creates a new `ChunkBatcherConfig` using the default values, optionally overridden
    /// through the environment.
    ///
    /// See [`Self::apply_env`].
    #[inline]
    pub fn from_env() -> ChunkBatcherResult<Self> {
        Self::default().apply_env()
    }

    /// Returns a copy of `self`, overriding existing fields with values from the environment if
    /// they are present.
    ///
    /// See [`Self::ENV_FLUSH_TICK`], [`Self::ENV_FLUSH_NUM_BYTES`], [`Self::ENV_FLUSH_NUM_BYTES`].
    pub fn apply_env(&self) -> ChunkBatcherResult<Self> {
        let mut new = self.clone();

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_TICK) {
            let flush_duration_secs: f64 =
                s.parse().map_err(|err| ChunkBatcherError::ParseConfig {
                    name: Self::ENV_FLUSH_TICK,
                    value: s.clone(),
                    err: Box::new(err),
                })?;

            new.flush_tick = Duration::from_secs_f64(flush_duration_secs);
        }

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_NUM_BYTES) {
            if let Some(num_bytes) = re_format::parse_bytes(&s) {
                // e.g. "10MB"
                new.flush_num_bytes = num_bytes.unsigned_abs();
            } else {
                // Assume it's just an integer
                new.flush_num_bytes = s.parse().map_err(|err| ChunkBatcherError::ParseConfig {
                    name: Self::ENV_FLUSH_NUM_BYTES,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
            }
        }

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_NUM_ROWS) {
            new.flush_num_rows = s.parse().map_err(|err| ChunkBatcherError::ParseConfig {
                name: Self::ENV_FLUSH_NUM_ROWS,
                value: s.clone(),
                err: Box::new(err),
            })?;
        }

        if let Ok(s) = std::env::var(Self::ENV_CHUNK_MAX_ROWS_IF_UNSORTED) {
            new.chunk_max_rows_if_unsorted =
                s.parse().map_err(|err| ChunkBatcherError::ParseConfig {
                    name: Self::ENV_CHUNK_MAX_ROWS_IF_UNSORTED,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
        }

        // Deprecated
        #[allow(deprecated)]
        if let Ok(s) = std::env::var(Self::ENV_MAX_CHUNK_ROWS_IF_UNSORTED) {
            new.chunk_max_rows_if_unsorted =
                s.parse().map_err(|err| ChunkBatcherError::ParseConfig {
                    name: Self::ENV_MAX_CHUNK_ROWS_IF_UNSORTED,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
        }

        Ok(new)
    }
}

#[test]
fn chunk_batcher_config() {
    // Detect breaking changes in our environment variables.
    std::env::set_var("RERUN_FLUSH_TICK_SECS", "0.3");
    std::env::set_var("RERUN_FLUSH_NUM_BYTES", "42");
    std::env::set_var("RERUN_FLUSH_NUM_ROWS", "666");
    std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", "7777");

    let config = ChunkBatcherConfig::from_env().unwrap();
    let expected = ChunkBatcherConfig {
        flush_tick: Duration::from_millis(300),
        flush_num_bytes: 42,
        flush_num_rows: 666,
        chunk_max_rows_if_unsorted: 7777,
        ..Default::default()
    };
    assert_eq!(expected, config);

    std::env::set_var("RERUN_MAX_CHUNK_ROWS_IF_UNSORTED", "9999");

    let config = ChunkBatcherConfig::from_env().unwrap();
    let expected = ChunkBatcherConfig {
        flush_tick: Duration::from_millis(300),
        flush_num_bytes: 42,
        flush_num_rows: 666,
        chunk_max_rows_if_unsorted: 9999,
        ..Default::default()
    };
    assert_eq!(expected, config);
}

// ---

/// Implements an asynchronous batcher that coalesces [`PendingRow`]s into [`Chunk`]s based upon
/// the thresholds defined in the associated [`ChunkBatcherConfig`].
///
/// ## Batching vs. splitting
///
/// The batching process is triggered solely by time and space thresholds -- whichever is hit first.
/// This process will result in one big dataframe.
///
/// The splitting process will then run on top of that big dataframe, and split it further down
/// into smaller [`Chunk`]s.
/// Specifically, the dataframe will be splits into enough [`Chunk`]s so as to guarantee that:
/// * no chunk contains data for more than one entity path
/// * no chunk contains rows with different sets of timelines
/// * no chunk uses more than one datatype for a given component
/// * no chunk contains more rows than a pre-configured threshold if one or more timelines are unsorted
///
/// ## Multithreading and ordering
///
/// [`ChunkBatcher`] can be cheaply clone and used freely across any number of threads.
///
/// Internally, all operations are linearized into a pipeline:
/// - All operations sent by a given thread will take effect in the same exact order as that
///   thread originally sent them in, from its point of view.
/// - There isn't any well defined global order across multiple threads.
///
/// This means that e.g. flushing the pipeline ([`Self::flush_blocking`]) guarantees that all
/// previous data sent by the calling thread has been batched and sent down the channel returned
/// by [`ChunkBatcher::chunks`]; no more, no less.
///
/// ## Shutdown
///
/// The batcher can only be shutdown by dropping all instances of it, at which point it will
/// automatically take care of flushing any pending data that might remain in the pipeline.
///
/// Shutting down cannot ever block.
#[derive(Clone)]
pub struct ChunkBatcher {
    inner: Arc<ChunkBatcherInner>,
}

// NOTE: The receiving end of the command stream as well as the sending end of the chunk stream are
// owned solely by the batching thread.
struct ChunkBatcherInner {
    /// The one and only entrypoint into the pipeline: this is _never_ cloned nor publicly exposed,
    /// therefore the `Drop` implementation is guaranteed that no more data can come in while it's
    /// running.
    tx_cmds: Sender<Command>,
    // NOTE: Option so we can make shutdown non-blocking even with bounded channels.
    rx_chunks: Option<Receiver<Chunk>>,
    cmds_to_chunks_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ChunkBatcherInner {
    fn drop(&mut self) {
        // Drop the receiving end of the chunk stream first and foremost, so that we don't block
        // even if the output channel is bounded and currently full.
        if let Some(rx_chunks) = self.rx_chunks.take() {
            if !rx_chunks.is_empty() {
                re_log::warn!("Dropping data");
            }
        }

        // NOTE: The command channel is private, if we're here, nothing is currently capable of
        // sending data down the pipeline.
        self.tx_cmds.send(Command::Shutdown).ok();
        if let Some(handle) = self.cmds_to_chunks_handle.take() {
            handle.join().ok();
        }
    }
}

enum Command {
    AppendChunk(Chunk),
    AppendRow(EntityPath, PendingRow),
    Flush(Sender<()>),
    Shutdown,
}

impl Command {
    fn flush() -> (Self, Receiver<()>) {
        let (tx, rx) = crossbeam::channel::bounded(0); // oneshot
        (Self::Flush(tx), rx)
    }
}

impl ChunkBatcher {
    /// Creates a new [`ChunkBatcher`] using the passed in `config`.
    ///
    /// The returned object must be kept in scope: dropping it will trigger a clean shutdown of the
    /// batcher.
    #[must_use = "Batching threads will automatically shutdown when this object is dropped"]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(config: ChunkBatcherConfig) -> ChunkBatcherResult<Self> {
        let (tx_cmds, rx_cmd) = match config.max_commands_in_flight {
            Some(cap) => crossbeam::channel::bounded(cap as _),
            None => crossbeam::channel::unbounded(),
        };

        let (tx_chunk, rx_chunks) = match config.max_chunks_in_flight {
            Some(cap) => crossbeam::channel::bounded(cap as _),
            None => crossbeam::channel::unbounded(),
        };

        let cmds_to_chunks_handle = {
            const NAME: &str = "ChunkBatcher::cmds_to_chunks";
            std::thread::Builder::new()
                .name(NAME.into())
                .spawn({
                    let config = config.clone();
                    move || batching_thread(config, rx_cmd, tx_chunk)
                })
                .map_err(|err| ChunkBatcherError::SpawnThread {
                    name: NAME,
                    err: Box::new(err),
                })?
        };

        re_log::debug!(?config, "creating new chunk batcher");

        let inner = ChunkBatcherInner {
            tx_cmds,
            rx_chunks: Some(rx_chunks),
            cmds_to_chunks_handle: Some(cmds_to_chunks_handle),
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    // --- Send commands ---

    pub fn push_chunk(&self, chunk: Chunk) {
        self.inner.push_chunk(chunk);
    }

    /// Pushes a [`PendingRow`] down the batching pipeline.
    ///
    /// This will computea the size of the row from the batching thread!
    ///
    /// See [`ChunkBatcher`] docs for ordering semantics and multithreading guarantees.
    #[inline]
    pub fn push_row(&self, entity_path: EntityPath, row: PendingRow) {
        self.inner.push_row(entity_path, row);
    }

    /// Initiates a flush of the pipeline and returns immediately.
    ///
    /// This does **not** wait for the flush to propagate (see [`Self::flush_blocking`]).
    /// See [`ChunkBatcher`] docs for ordering semantics and multithreading guarantees.
    #[inline]
    pub fn flush_async(&self) {
        self.inner.flush_async();
    }

    /// Initiates a flush the batching pipeline and waits for it to propagate.
    ///
    /// See [`ChunkBatcher`] docs for ordering semantics and multithreading guarantees.
    #[inline]
    pub fn flush_blocking(&self) {
        self.inner.flush_blocking();
    }

    // --- Subscribe to chunks ---

    /// Returns a _shared_ channel in which are sent the batched [`Chunk`]s.
    ///
    /// Shutting down the batcher will close this channel.
    ///
    /// See [`ChunkBatcher`] docs for ordering semantics and multithreading guarantees.
    pub fn chunks(&self) -> Receiver<Chunk> {
        // NOTE: `rx_chunks` is only ever taken when the batcher as a whole is dropped, at which
        // point it is impossible to call this method.
        #[allow(clippy::unwrap_used)]
        self.inner.rx_chunks.clone().unwrap()
    }
}

impl ChunkBatcherInner {
    fn push_chunk(&self, chunk: Chunk) {
        self.send_cmd(Command::AppendChunk(chunk));
    }

    fn push_row(&self, entity_path: EntityPath, row: PendingRow) {
        self.send_cmd(Command::AppendRow(entity_path, row));
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
fn batching_thread(config: ChunkBatcherConfig, rx_cmd: Receiver<Command>, tx_chunk: Sender<Chunk>) {
    let rx_tick = crossbeam::channel::tick(config.flush_tick);

    struct Accumulator {
        latest: Instant,
        entity_path: EntityPath,
        pending_rows: Vec<PendingRow>,
        pending_num_bytes: u64,
    }

    impl Accumulator {
        fn new(entity_path: EntityPath) -> Self {
            Self {
                entity_path,
                latest: Instant::now(),
                pending_rows: Default::default(),
                pending_num_bytes: Default::default(),
            }
        }

        fn reset(&mut self) {
            self.latest = Instant::now();
            self.pending_rows.clear();
            self.pending_num_bytes = 0;
        }
    }

    let mut accs: IntMap<EntityPath, Accumulator> = IntMap::default();

    fn do_push_row(acc: &mut Accumulator, row: PendingRow) {
        acc.pending_num_bytes += row.total_size_bytes();
        acc.pending_rows.push(row);
    }

    fn do_flush_all(
        acc: &mut Accumulator,
        tx_chunk: &Sender<Chunk>,
        reason: &str,
        chunk_max_rows_if_unsorted: u64,
    ) {
        let rows = std::mem::take(&mut acc.pending_rows);
        if rows.is_empty() {
            return;
        }

        re_log::trace!(
            "Flushing {} rows and {} bytes. Reason: {reason}",
            rows.len(),
            re_format::format_bytes(acc.pending_num_bytes as _)
        );

        let chunks =
            PendingRow::many_into_chunks(acc.entity_path.clone(), chunk_max_rows_if_unsorted, rows);
        for chunk in chunks {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(err) => {
                    re_log::error!(%err, "corrupt chunk detected, dropping");
                    continue;
                }
            };

            // NOTE: This can only fail if all receivers have been dropped, which simply cannot happen
            // as long the batching thread is alive… which is where we currently are.
            tx_chunk.send(chunk).ok();
        }

        acc.reset();
    }

    re_log::trace!(
        "Flushing every: {:.2}s, {} rows, {}",
        config.flush_tick.as_secs_f64(),
        config.flush_num_rows,
        re_format::format_bytes(config.flush_num_bytes as _),
    );

    // Set to `true` when a flush is triggered for a reason other than hitting the time threshold,
    // so that the next tick will not unnecessarily fire early.
    let mut skip_next_tick = false;

    use crossbeam::select;
    loop {
        select! {
            recv(rx_cmd) -> cmd => {
                let Ok(cmd) = cmd else {
                    // All command senders are gone, which can only happen if the
                    // `ChunkBatcher` itself has been dropped.
                    break;
                };


                match cmd {
                    Command::AppendChunk(chunk) => {
                        // NOTE: This can only fail if all receivers have been dropped, which simply cannot happen
                        // as long the batching thread is alive… which is where we currently are.
                        tx_chunk.send(chunk).ok();
                    },
                    Command::AppendRow(entity_path, row) => {
                        let acc = accs.entry(entity_path.clone())
                            .or_insert_with(|| Accumulator::new(entity_path));
                        do_push_row(acc, row);

                        if let Some(config) = config.hooks.on_insert.as_ref() {
                            config(&acc.pending_rows);
                        }

                        if acc.pending_rows.len() as u64 >= config.flush_num_rows {
                            do_flush_all(acc, &tx_chunk, "rows", config.chunk_max_rows_if_unsorted);
                            skip_next_tick = true;
                        } else if acc.pending_num_bytes >= config.flush_num_bytes {
                            do_flush_all(acc, &tx_chunk, "bytes", config.chunk_max_rows_if_unsorted);
                            skip_next_tick = true;
                        }
                    },

                    Command::Flush(oneshot) => {
                        skip_next_tick = true;
                        for acc in accs.values_mut() {
                            do_flush_all(acc, &tx_chunk, "manual", config.chunk_max_rows_if_unsorted);
                        }
                        drop(oneshot); // signals the oneshot
                    },

                    Command::Shutdown => break,
                };
            },

            recv(rx_tick) -> _ => {
                if skip_next_tick {
                    skip_next_tick = false;
                } else {
                    // TODO(cmc): It would probably be better to have a ticker per entity path. Maybe. At some point.
                    for acc in accs.values_mut() {
                        do_flush_all(acc, &tx_chunk, "tick", config.chunk_max_rows_if_unsorted);
                    }
                }
            },
        };
    }

    drop(rx_cmd);
    for acc in accs.values_mut() {
        do_flush_all(
            acc,
            &tx_chunk,
            "shutdown",
            config.chunk_max_rows_if_unsorted,
        );
    }
    drop(tx_chunk);

    // NOTE: The receiving end of the command stream as well as the sending end of the chunk
    // stream are owned solely by this thread.
    // Past this point, all command writes and all chunk reads will return `ErrDisconnected`.
}

// ---

/// A single row's worth of data (i.e. a single log call).
///
/// Send those to the batcher to build up a [`Chunk`].
#[derive(Debug, Clone)]
pub struct PendingRow {
    /// Auto-generated `TUID`, uniquely identifying this event and keeping track of the client's
    /// wall-clock.
    pub row_id: RowId,

    /// User-specified [`TimePoint`] for this event.
    pub timepoint: TimePoint,

    /// The component data.
    ///
    /// Each array is a single component, i.e. _not_ a list array.
    pub components: IntMap<ComponentDescriptor, Box<dyn Arrow2Array>>,
}

impl PendingRow {
    #[inline]
    pub fn new(
        timepoint: TimePoint,
        components: IntMap<ComponentDescriptor, Box<dyn Arrow2Array>>,
    ) -> Self {
        Self {
            row_id: RowId::new(),
            timepoint,
            components,
        }
    }
}

impl re_byte_size::SizeBytes for PendingRow {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            row_id,
            timepoint,
            components,
        } = self;

        row_id.heap_size_bytes() + timepoint.heap_size_bytes() + components.heap_size_bytes()
    }
}

impl PendingRow {
    /// Turn a single row into a [`Chunk`] of its own.
    ///
    /// That's very wasteful, probably don't do that outside of testing, or unless you have very
    /// good reasons too.
    ///
    /// See also [`Self::many_into_chunks`].
    pub fn into_chunk(self, entity_path: EntityPath) -> ChunkResult<Chunk> {
        let Self {
            row_id,
            timepoint,
            components,
        } = self;

        let timelines = timepoint
            .into_iter()
            .map(|(timeline, time)| {
                let times = Arrow2PrimitiveArray::<i64>::from_vec(vec![time.as_i64()]);
                let time_column = TimeColumn::new(Some(true), timeline, times);
                (timeline, time_column)
            })
            .collect();

        let mut per_name = ChunkComponents::default();
        for (component_desc, array) in components {
            let list_array = crate::util::arrays_to_list_array_opt(&[Some(&*array as _)]);
            if let Some(list_array) = list_array {
                per_name.insert_descriptor(component_desc, list_array);
            }
        }

        Chunk::from_native_row_ids(
            ChunkId::new(),
            entity_path,
            Some(true),
            &[row_id],
            timelines,
            per_name,
        )
    }

    /// This turns a batch of [`PendingRow`]s into a [`Chunk`].
    ///
    /// There are a lot of conditions to fulfill for a [`Chunk`] to be valid: this helper makes
    /// sure to fulfill all of them by splitting the chunk into one or more pieces as necessary.
    ///
    /// In particular, a [`Chunk`] cannot:
    /// * contain data for more than one entity path
    /// * contain rows with different sets of timelines
    /// * use more than one datatype for a given component
    /// * contain more rows than a pre-configured threshold if one or more timelines are unsorted
    //
    // TODO(cmc): there are lots of performance improvement opportunities in this one, but let's
    // see if that actually matters in practice first.
    pub fn many_into_chunks(
        entity_path: EntityPath,
        chunk_max_rows_if_unsorted: u64,
        mut rows: Vec<Self>,
    ) -> impl Iterator<Item = ChunkResult<Chunk>> {
        re_tracing::profile_function!();

        // First things first, sort all the rows by row ID -- that's our global order and it holds
        // true no matter what.
        {
            re_tracing::profile_scope!("sort rows");
            rows.sort_by_key(|row| row.row_id);
        }

        // Then organize the rows in micro batches -- one batch per unique set of timelines.
        let mut per_timeline_set: IntMap<u64 /* Timeline set */, Vec<Self>> = Default::default();
        {
            re_tracing::profile_scope!("compute timeline sets");

            for row in rows {
                let mut hasher = ahash::AHasher::default();
                row.timepoint
                    .timelines()
                    .for_each(|timeline| timeline.hash(&mut hasher));

                per_timeline_set
                    .entry(hasher.finish())
                    .or_default()
                    .push(row);
            }
        }

        per_timeline_set.into_values().flat_map(move |rows| {
            re_tracing::profile_scope!("iterate per timeline set");

            // Then we split the micro batches even further -- one sub-batch per unique set of datatypes.
            let mut per_datatype_set: IntMap<u64 /* Arrow2Datatype set */, Vec<Self>> =
                Default::default();
            {
                re_tracing::profile_scope!("compute datatype sets");

                for row in rows {
                    let mut hasher = ahash::AHasher::default();
                    row.components
                        .values()
                        .for_each(|array| array.data_type().hash(&mut hasher));
                    per_datatype_set
                        .entry(hasher.finish())
                        .or_default()
                        .push(row);
                }
            }

            // And finally we can build the resulting chunks.
            let entity_path = entity_path.clone();
            per_datatype_set.into_values().flat_map(move |rows| {
                re_tracing::profile_scope!("iterate per datatype set");

                let mut row_ids: Vec<RowId> = Vec::with_capacity(rows.len());
                let mut timelines: IntMap<Timeline, PendingTimeColumn> = IntMap::default();

                // Create all the logical list arrays that we're going to need, accounting for the
                // possibility of sparse components in the data.
                let mut all_components: IntMap<ComponentDescriptor, Vec<Option<&dyn Arrow2Array>>> =
                    IntMap::default();
                for row in &rows {
                    for component_desc in row.components.keys() {
                        all_components.entry(component_desc.clone()).or_default();
                    }
                }

                let mut chunks = Vec::new();

                let mut components = all_components.clone();
                for row in &rows {
                    let Self {
                        row_id,
                        timepoint: row_timepoint,
                        components: row_components,
                    } = row;

                    // Look for unsorted timelines -- if we find any, and the chunk is larger than
                    // the pre-configured `chunk_max_rows_if_unsorted` threshold, then split _even_
                    // further!
                    for (&timeline, _) in row_timepoint {
                        let time_column = timelines
                            .entry(timeline)
                            .or_insert_with(|| PendingTimeColumn::new(timeline));

                        if !row_ids.is_empty() // just being extra cautious
                            && row_ids.len() as u64 >= chunk_max_rows_if_unsorted
                            && !time_column.is_sorted
                        {
                            chunks.push(Chunk::from_native_row_ids(
                                ChunkId::new(),
                                entity_path.clone(),
                                Some(true),
                                &std::mem::take(&mut row_ids),
                                std::mem::take(&mut timelines)
                                    .into_iter()
                                    .map(|(timeline, time_column)| (timeline, time_column.finish()))
                                    .collect(),
                                {
                                    let mut per_name = ChunkComponents::default();
                                    for (component_desc, arrays) in std::mem::take(&mut components)
                                    {
                                        let list_array =
                                            crate::util::arrays_to_list_array_opt(&arrays);
                                        if let Some(list_array) = list_array {
                                            per_name.insert_descriptor(component_desc, list_array);
                                        }
                                    }
                                    per_name
                                },
                            ));

                            components = all_components.clone();
                        }
                    }

                    row_ids.push(*row_id);

                    for (&timeline, &time) in row_timepoint {
                        let time_column = timelines
                            .entry(timeline)
                            .or_insert_with(|| PendingTimeColumn::new(timeline));
                        time_column.push(time);
                    }

                    for (component_desc, arrays) in &mut components {
                        // NOTE: This will push `None` if the row doesn't actually hold a value for this
                        // component -- these are sparse list arrays!
                        arrays.push(
                            row_components
                                .get(component_desc)
                                .map(|array| &**array as &dyn Arrow2Array),
                        );
                    }
                }

                chunks.push(Chunk::from_native_row_ids(
                    ChunkId::new(),
                    entity_path.clone(),
                    Some(true),
                    &std::mem::take(&mut row_ids),
                    timelines
                        .into_iter()
                        .map(|(timeline, time_column)| (timeline, time_column.finish()))
                        .collect(),
                    {
                        let mut per_name = ChunkComponents::default();
                        for (component_desc, arrays) in components {
                            let list_array = crate::util::arrays_to_list_array_opt(&arrays);
                            if let Some(list_array) = list_array {
                                per_name.insert_descriptor(component_desc, list_array);
                            }
                        }
                        per_name
                    },
                ));

                chunks
            })
        })
    }
}

/// Helper class used to buffer time data.
///
/// See [`PendingRow::many_into_chunks`] for usage.
struct PendingTimeColumn {
    timeline: Timeline,
    times: Vec<i64>,
    is_sorted: bool,
    time_range: ResolvedTimeRange,
}

impl PendingTimeColumn {
    fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            times: Default::default(),
            is_sorted: true,
            time_range: ResolvedTimeRange::EMPTY,
        }
    }

    /// Push a single time value at the end of this chunk.
    fn push(&mut self, time: TimeInt) {
        let Self {
            timeline: _,
            times,
            is_sorted,
            time_range,
        } = self;

        *is_sorted &= times.last().copied().unwrap_or(TimeInt::MIN.as_i64()) <= time.as_i64();
        time_range.set_min(TimeInt::min(time_range.min(), time));
        time_range.set_max(TimeInt::max(time_range.max(), time));
        times.push(time.as_i64());
    }

    fn finish(self) -> TimeColumn {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range,
        } = self;

        TimeColumn {
            timeline,
            times: Arrow2PrimitiveArray::<i64>::from_vec(times).to(timeline.datatype()),
            is_sorted,
            time_range,
        }
    }
}

// ---

// NOTE:
// These tests only cover the chunk splitting conditions described in `many_into_chunks`.
// Temporal and spatial thresholds are already taken care of by the RecordingStream test suite.

#[cfg(test)]
mod tests {
    use crossbeam::channel::TryRecvError;

    use re_log_types::example_components::{MyPoint, MyPoint64};
    use re_types_core::{Component as _, Loggable as _};

    use super::*;

    /// A bunch of rows that don't fit any of the split conditions should end up together.
    #[test]
    fn simple() -> anyhow::Result<()> {
        let batcher = ChunkBatcher::new(ChunkBatcherConfig::NEVER)?;

        let timeline1 = Timeline::new_temporal("log_time");

        let timepoint1 = TimePoint::default().with(timeline1, 42);
        let timepoint2 = TimePoint::default().with(timeline1, 43);
        let timepoint3 = TimePoint::default().with(timeline1, 44);

        let points1 = MyPoint::to_arrow2([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])?;
        let points2 = MyPoint::to_arrow2([MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)])?;
        let points3 = MyPoint::to_arrow2([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;

        let components1 = [(MyPoint::descriptor(), points1.clone())];
        let components2 = [(MyPoint::descriptor(), points2.clone())];
        let components3 = [(MyPoint::descriptor(), points3.clone())];

        let row1 = PendingRow::new(timepoint1.clone(), components1.into_iter().collect());
        let row2 = PendingRow::new(timepoint2.clone(), components2.into_iter().collect());
        let row3 = PendingRow::new(timepoint3.clone(), components3.into_iter().collect());

        let entity_path1: EntityPath = "a/b/c".into();
        batcher.push_row(entity_path1.clone(), row1.clone());
        batcher.push_row(entity_path1.clone(), row2.clone());
        batcher.push_row(entity_path1.clone(), row3.clone());

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        let mut chunks = Vec::new();
        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            chunks.push(chunk);
        }

        chunks.sort_by_key(|chunk| chunk.row_id_range().unwrap().0);

        // Make the programmer's life easier if this test fails.
        eprintln!("Chunks:");
        for chunk in &chunks {
            eprintln!("{chunk}");
        }

        assert_eq!(1, chunks.len());

        {
            let expected_row_ids = vec![row1.row_id, row2.row_id, row3.row_id];
            let expected_timelines = [(
                timeline1,
                TimeColumn::new(
                    Some(true),
                    timeline1,
                    Arrow2PrimitiveArray::from_vec(vec![42, 43, 44]),
                ),
            )];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points1, &*points2, &*points3].map(Some))
                    .unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[0].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[0]);
            assert_eq!(expected_chunk, chunks[0]);
        }

        Ok(())
    }

    /// A bunch of rows that don't fit any of the split conditions should end up together.
    #[test]
    fn simple_static() -> anyhow::Result<()> {
        let batcher = ChunkBatcher::new(ChunkBatcherConfig::NEVER)?;

        let timeless = TimePoint::default();

        let points1 = MyPoint::to_arrow2([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])?;
        let points2 = MyPoint::to_arrow2([MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)])?;
        let points3 = MyPoint::to_arrow2([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;

        let components1 = [(MyPoint::descriptor(), points1.clone())];
        let components2 = [(MyPoint::descriptor(), points2.clone())];
        let components3 = [(MyPoint::descriptor(), points3.clone())];

        let row1 = PendingRow::new(timeless.clone(), components1.into_iter().collect());
        let row2 = PendingRow::new(timeless.clone(), components2.into_iter().collect());
        let row3 = PendingRow::new(timeless.clone(), components3.into_iter().collect());

        let entity_path1: EntityPath = "a/b/c".into();
        batcher.push_row(entity_path1.clone(), row1.clone());
        batcher.push_row(entity_path1.clone(), row2.clone());
        batcher.push_row(entity_path1.clone(), row3.clone());

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        let mut chunks = Vec::new();
        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            chunks.push(chunk);
        }

        chunks.sort_by_key(|chunk| chunk.row_id_range().unwrap().0);

        // Make the programmer's life easier if this test fails.
        eprintln!("Chunks:");
        for chunk in &chunks {
            eprintln!("{chunk}");
        }

        assert_eq!(1, chunks.len());

        {
            let expected_row_ids = vec![row1.row_id, row2.row_id, row3.row_id];
            let expected_timelines = [];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points1, &*points2, &*points3].map(Some))
                    .unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[0].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[0]);
            assert_eq!(expected_chunk, chunks[0]);
        }

        Ok(())
    }

    /// A bunch of rows belonging to different entities will end up in different batches.
    #[test]
    fn different_entities() -> anyhow::Result<()> {
        let batcher = ChunkBatcher::new(ChunkBatcherConfig::NEVER)?;

        let timeline1 = Timeline::new_temporal("log_time");

        let timepoint1 = TimePoint::default().with(timeline1, 42);
        let timepoint2 = TimePoint::default().with(timeline1, 43);
        let timepoint3 = TimePoint::default().with(timeline1, 44);

        let points1 = MyPoint::to_arrow2([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])?;
        let points2 = MyPoint::to_arrow2([MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)])?;
        let points3 = MyPoint::to_arrow2([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;

        let components1 = [(MyPoint::descriptor(), points1.clone())];
        let components2 = [(MyPoint::descriptor(), points2.clone())];
        let components3 = [(MyPoint::descriptor(), points3.clone())];

        let row1 = PendingRow::new(timepoint1.clone(), components1.into_iter().collect());
        let row2 = PendingRow::new(timepoint2.clone(), components2.into_iter().collect());
        let row3 = PendingRow::new(timepoint3.clone(), components3.into_iter().collect());

        let entity_path1: EntityPath = "ent1".into();
        let entity_path2: EntityPath = "ent2".into();
        batcher.push_row(entity_path1.clone(), row1.clone());
        batcher.push_row(entity_path2.clone(), row2.clone());
        batcher.push_row(entity_path1.clone(), row3.clone());

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        let mut chunks = Vec::new();
        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            chunks.push(chunk);
        }

        chunks.sort_by_key(|chunk| chunk.row_id_range().unwrap().0);

        // Make the programmer's life easier if this test fails.
        eprintln!("Chunks:");
        for chunk in &chunks {
            eprintln!("{chunk}");
        }

        assert_eq!(2, chunks.len());

        {
            let expected_row_ids = vec![row1.row_id, row3.row_id];
            let expected_timelines = [(
                timeline1,
                TimeColumn::new(
                    Some(true),
                    timeline1,
                    Arrow2PrimitiveArray::from_vec(vec![42, 44]),
                ),
            )];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points1, &*points3].map(Some)).unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[0].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[0]);
            assert_eq!(expected_chunk, chunks[0]);
        }

        {
            let expected_row_ids = vec![row2.row_id];
            let expected_timelines = [(
                timeline1,
                TimeColumn::new(
                    Some(true),
                    timeline1,
                    Arrow2PrimitiveArray::from_vec(vec![43]),
                ),
            )];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points2].map(Some)).unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[1].id,
                entity_path2.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[1]);
            assert_eq!(expected_chunk, chunks[1]);
        }

        Ok(())
    }

    /// A bunch of rows with different sets of timelines will end up in different batches.
    #[test]
    fn different_timelines() -> anyhow::Result<()> {
        let batcher = ChunkBatcher::new(ChunkBatcherConfig::NEVER)?;

        let timeline1 = Timeline::new_temporal("log_time");
        let timeline2 = Timeline::new_sequence("frame_nr");

        let timepoint1 = TimePoint::default().with(timeline1, 42);
        let timepoint2 = TimePoint::default()
            .with(timeline1, 43)
            .with(timeline2, 1000);
        let timepoint3 = TimePoint::default()
            .with(timeline1, 44)
            .with(timeline2, 1001);

        let points1 = MyPoint::to_arrow2([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])?;
        let points2 = MyPoint::to_arrow2([MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)])?;
        let points3 = MyPoint::to_arrow2([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;

        let components1 = [(MyPoint::descriptor(), points1.clone())];
        let components2 = [(MyPoint::descriptor(), points2.clone())];
        let components3 = [(MyPoint::descriptor(), points3.clone())];

        let row1 = PendingRow::new(timepoint1.clone(), components1.into_iter().collect());
        let row2 = PendingRow::new(timepoint2.clone(), components2.into_iter().collect());
        let row3 = PendingRow::new(timepoint3.clone(), components3.into_iter().collect());

        let entity_path1: EntityPath = "a/b/c".into();
        batcher.push_row(entity_path1.clone(), row1.clone());
        batcher.push_row(entity_path1.clone(), row2.clone());
        batcher.push_row(entity_path1.clone(), row3.clone());

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        let mut chunks = Vec::new();
        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            chunks.push(chunk);
        }

        chunks.sort_by_key(|chunk| chunk.row_id_range().unwrap().0);

        // Make the programmer's life easier if this test fails.
        eprintln!("Chunks:");
        for chunk in &chunks {
            eprintln!("{chunk}");
        }

        assert_eq!(2, chunks.len());

        {
            let expected_row_ids = vec![row1.row_id];
            let expected_timelines = [(
                timeline1,
                TimeColumn::new(
                    Some(true),
                    timeline1,
                    Arrow2PrimitiveArray::from_vec(vec![42]),
                ),
            )];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points1].map(Some)).unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[0].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[0]);
            assert_eq!(expected_chunk, chunks[0]);
        }

        {
            let expected_row_ids = vec![row2.row_id, row3.row_id];
            let expected_timelines = [
                (
                    timeline1,
                    TimeColumn::new(
                        Some(true),
                        timeline1,
                        Arrow2PrimitiveArray::from_vec(vec![43, 44]),
                    ),
                ),
                (
                    timeline2,
                    TimeColumn::new(
                        Some(true),
                        timeline2,
                        Arrow2PrimitiveArray::from_vec(vec![1000, 1001]),
                    ),
                ),
            ];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points2, &*points3].map(Some)).unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[1].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[1]);
            assert_eq!(expected_chunk, chunks[1]);
        }

        Ok(())
    }

    /// A bunch of rows with different datatypes will end up in different batches.
    #[test]
    fn different_datatypes() -> anyhow::Result<()> {
        let batcher = ChunkBatcher::new(ChunkBatcherConfig::NEVER)?;

        let timeline1 = Timeline::new_temporal("log_time");

        let timepoint1 = TimePoint::default().with(timeline1, 42);
        let timepoint2 = TimePoint::default().with(timeline1, 43);
        let timepoint3 = TimePoint::default().with(timeline1, 44);

        let points1 = MyPoint::to_arrow2([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])?;
        let points2 =
            MyPoint64::to_arrow2([MyPoint64::new(10.0, 20.0), MyPoint64::new(30.0, 40.0)])?;
        let points3 = MyPoint::to_arrow2([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;

        let components1 = [(MyPoint::descriptor(), points1.clone())];
        let components2 = [(MyPoint::descriptor(), points2.clone())]; // same name, different datatype
        let components3 = [(MyPoint::descriptor(), points3.clone())];

        let row1 = PendingRow::new(timepoint1.clone(), components1.into_iter().collect());
        let row2 = PendingRow::new(timepoint2.clone(), components2.into_iter().collect());
        let row3 = PendingRow::new(timepoint3.clone(), components3.into_iter().collect());

        let entity_path1: EntityPath = "a/b/c".into();
        batcher.push_row(entity_path1.clone(), row1.clone());
        batcher.push_row(entity_path1.clone(), row2.clone());
        batcher.push_row(entity_path1.clone(), row3.clone());

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        let mut chunks = Vec::new();
        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            chunks.push(chunk);
        }

        chunks.sort_by_key(|chunk| chunk.row_id_range().unwrap().0);

        // Make the programmer's life easier if this test fails.
        eprintln!("Chunks:");
        for chunk in &chunks {
            eprintln!("{chunk}");
        }

        assert_eq!(2, chunks.len());

        {
            let expected_row_ids = vec![row1.row_id, row3.row_id];
            let expected_timelines = [(
                timeline1,
                TimeColumn::new(
                    Some(true),
                    timeline1,
                    Arrow2PrimitiveArray::from_vec(vec![42, 44]),
                ),
            )];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points1, &*points3].map(Some)).unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[0].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[0]);
            assert_eq!(expected_chunk, chunks[0]);
        }

        {
            let expected_row_ids = vec![row2.row_id];
            let expected_timelines = [(
                timeline1,
                TimeColumn::new(
                    Some(true),
                    timeline1,
                    Arrow2PrimitiveArray::from_vec(vec![43]),
                ),
            )];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points2].map(Some)).unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[1].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[1]);
            assert_eq!(expected_chunk, chunks[1]);
        }

        Ok(())
    }

    /// If one or more of the timelines end up unsorted, but the batch is below the unsorted length
    /// threshold, we don't do anything special.
    #[test]
    fn unsorted_timeline_below_threshold() -> anyhow::Result<()> {
        let batcher = ChunkBatcher::new(ChunkBatcherConfig {
            chunk_max_rows_if_unsorted: 1000,
            ..ChunkBatcherConfig::NEVER
        })?;

        let timeline1 = Timeline::new_temporal("log_time");
        let timeline2 = Timeline::new_temporal("frame_nr");

        let timepoint1 = TimePoint::default()
            .with(timeline2, 1000)
            .with(timeline1, 42);
        let timepoint2 = TimePoint::default()
            .with(timeline2, 1001)
            .with(timeline1, 43);
        let timepoint3 = TimePoint::default()
            .with(timeline2, 1002)
            .with(timeline1, 44);
        let timepoint4 = TimePoint::default()
            .with(timeline2, 1003)
            .with(timeline1, 45);

        let points1 = MyPoint::to_arrow2([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])?;
        let points2 = MyPoint::to_arrow2([MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)])?;
        let points3 = MyPoint::to_arrow2([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;
        let points4 =
            MyPoint::to_arrow2([MyPoint::new(1000.0, 2000.0), MyPoint::new(3000.0, 4000.0)])?;

        let components1 = [(MyPoint::descriptor(), points1.clone())];
        let components2 = [(MyPoint::descriptor(), points2.clone())];
        let components3 = [(MyPoint::descriptor(), points3.clone())];
        let components4 = [(MyPoint::descriptor(), points4.clone())];

        let row1 = PendingRow::new(timepoint4.clone(), components1.into_iter().collect());
        let row2 = PendingRow::new(timepoint1.clone(), components2.into_iter().collect());
        let row3 = PendingRow::new(timepoint2.clone(), components3.into_iter().collect());
        let row4 = PendingRow::new(timepoint3.clone(), components4.into_iter().collect());

        let entity_path1: EntityPath = "a/b/c".into();
        batcher.push_row(entity_path1.clone(), row1.clone());
        batcher.push_row(entity_path1.clone(), row2.clone());
        batcher.push_row(entity_path1.clone(), row3.clone());
        batcher.push_row(entity_path1.clone(), row4.clone());

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        let mut chunks = Vec::new();
        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            chunks.push(chunk);
        }

        chunks.sort_by_key(|chunk| chunk.row_id_range().unwrap().0);

        // Make the programmer's life easier if this test fails.
        eprintln!("Chunks:");
        for chunk in &chunks {
            eprintln!("{chunk}");
        }

        assert_eq!(1, chunks.len());

        {
            let expected_row_ids = vec![row1.row_id, row2.row_id, row3.row_id, row4.row_id];
            let expected_timelines = [
                (
                    timeline1,
                    TimeColumn::new(
                        Some(false),
                        timeline1,
                        Arrow2PrimitiveArray::from_vec(vec![45, 42, 43, 44]),
                    ),
                ),
                (
                    timeline2,
                    TimeColumn::new(
                        Some(false),
                        timeline2,
                        Arrow2PrimitiveArray::from_vec(vec![1003, 1000, 1001, 1002]),
                    ),
                ),
            ];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(
                    &[&*points1, &*points2, &*points3, &*points4].map(Some),
                )
                .unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[0].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[0]);
            assert_eq!(expected_chunk, chunks[0]);
        }

        Ok(())
    }

    /// If one or more of the timelines end up unsorted, and the batch is above the unsorted length
    /// threshold, we split it.
    #[test]
    fn unsorted_timeline_above_threshold() -> anyhow::Result<()> {
        let batcher = ChunkBatcher::new(ChunkBatcherConfig {
            chunk_max_rows_if_unsorted: 3,
            ..ChunkBatcherConfig::NEVER
        })?;

        let timeline1 = Timeline::new_temporal("log_time");
        let timeline2 = Timeline::new_temporal("frame_nr");

        let timepoint1 = TimePoint::default()
            .with(timeline2, 1000)
            .with(timeline1, 42);
        let timepoint2 = TimePoint::default()
            .with(timeline2, 1001)
            .with(timeline1, 43);
        let timepoint3 = TimePoint::default()
            .with(timeline2, 1002)
            .with(timeline1, 44);
        let timepoint4 = TimePoint::default()
            .with(timeline2, 1003)
            .with(timeline1, 45);

        let points1 = MyPoint::to_arrow2([MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)])?;
        let points2 = MyPoint::to_arrow2([MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)])?;
        let points3 = MyPoint::to_arrow2([MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)])?;
        let points4 =
            MyPoint::to_arrow2([MyPoint::new(1000.0, 2000.0), MyPoint::new(3000.0, 4000.0)])?;

        let components1 = [(MyPoint::descriptor(), points1.clone())];
        let components2 = [(MyPoint::descriptor(), points2.clone())];
        let components3 = [(MyPoint::descriptor(), points3.clone())];
        let components4 = [(MyPoint::descriptor(), points4.clone())];

        let row1 = PendingRow::new(timepoint4.clone(), components1.into_iter().collect());
        let row2 = PendingRow::new(timepoint1.clone(), components2.into_iter().collect());
        let row3 = PendingRow::new(timepoint2.clone(), components3.into_iter().collect());
        let row4 = PendingRow::new(timepoint3.clone(), components4.into_iter().collect());

        let entity_path1: EntityPath = "a/b/c".into();
        batcher.push_row(entity_path1.clone(), row1.clone());
        batcher.push_row(entity_path1.clone(), row2.clone());
        batcher.push_row(entity_path1.clone(), row3.clone());
        batcher.push_row(entity_path1.clone(), row4.clone());

        let chunks_rx = batcher.chunks();
        drop(batcher); // flush and close

        let mut chunks = Vec::new();
        loop {
            let chunk = match chunks_rx.try_recv() {
                Ok(chunk) => chunk,
                Err(TryRecvError::Empty) => panic!("expected chunk, got none"),
                Err(TryRecvError::Disconnected) => break,
            };
            chunks.push(chunk);
        }

        chunks.sort_by_key(|chunk| chunk.row_id_range().unwrap().0);

        // Make the programmer's life easier if this test fails.
        eprintln!("Chunks:");
        for chunk in &chunks {
            eprintln!("{chunk}");
        }

        assert_eq!(2, chunks.len());

        {
            let expected_row_ids = vec![row1.row_id, row2.row_id, row3.row_id];
            let expected_timelines = [
                (
                    timeline1,
                    TimeColumn::new(
                        Some(false),
                        timeline1,
                        Arrow2PrimitiveArray::from_vec(vec![45, 42, 43]),
                    ),
                ),
                (
                    timeline2,
                    TimeColumn::new(
                        Some(false),
                        timeline2,
                        Arrow2PrimitiveArray::from_vec(vec![1003, 1000, 1001]),
                    ),
                ),
            ];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points1, &*points2, &*points3].map(Some))
                    .unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[0].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[0]);
            assert_eq!(expected_chunk, chunks[0]);
        }

        {
            let expected_row_ids = vec![row4.row_id];
            let expected_timelines = [
                (
                    timeline1,
                    TimeColumn::new(
                        Some(true),
                        timeline1,
                        Arrow2PrimitiveArray::from_vec(vec![44]),
                    ),
                ),
                (
                    timeline2,
                    TimeColumn::new(
                        Some(true),
                        timeline2,
                        Arrow2PrimitiveArray::from_vec(vec![1002]),
                    ),
                ),
            ];
            let expected_components = [(
                MyPoint::descriptor(),
                crate::util::arrays_to_list_array_opt(&[&*points4].map(Some)).unwrap(),
            )];
            let expected_chunk = Chunk::from_native_row_ids(
                chunks[1].id,
                entity_path1.clone(),
                None,
                &expected_row_ids,
                expected_timelines.into_iter().collect(),
                expected_components.into_iter().collect(),
            )?;

            eprintln!("Expected:\n{expected_chunk}");
            eprintln!("Got:\n{}", chunks[1]);
            assert_eq!(expected_chunk, chunks[1]);
        }

        Ok(())
    }
}
