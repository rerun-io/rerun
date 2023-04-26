// TODO: debouncing is probably another PR?

// TODO: logs

// TODO: why at the table level rather than the chunk level you ask?

// TODO: we're still inserting row by row on the other end though

// TODO: exact number of rows seem to diverge a lil bit

// TODO: sorting will happen in another PR where we do it for both tables and the store path

// TODO: where's the missing data?

// TODO: test in rust, test in python...

// TODO: we have to be quite specific about per-process/per-thread/per-recording?

// TODO: strong guarantees for the thread that flushes/shutdowns/drops, weak guarantees for the
// other ones

// TODO: no global order

// TODO: offer a non-env based way to config for SDKs

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam::channel::{Receiver, SendError, Sender};
use nohash_hasher::IntMap;

use crate::{DataRow, DataTable, RecordingId, SizeBytes, TableId};

// ---

/// Errors that can occur when creating/manipulating a [`DataTableBatcher`].
#[derive(thiserror::Error, Debug)]
pub enum DataTableBatcherError {
    /// Error when parsing configuration.
    #[error("Failed to parse config: '{name}={value}': {err}")]
    ParseConfig {
        name: &'static str,
        value: String,
        err: Box<dyn std::error::Error>,
    },

    /// Error spawning one of the background threads.
    #[error("Failed to spawn background thread '{name}': {err}")]
    SpawnThread {
        name: &'static str,
        err: Box<dyn std::error::Error>,
    },
}

pub type DataTableBatcherResult<T> = Result<T, DataTableBatcherError>;

// ---

/// See [`Self::default`].
// TODO: doc, config when creating session
// TODO: debouncing support
// TODO: sort support
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataTableBatcherConfig {
    /// Duration of the periodic flush.
    //
    // NOTE: We use `std::time` directly because this library has to deal with crossbeam and
    // threads which expect standard types anyway.
    pub flush_duration: Duration,

    /// Flush if the accumulated payload has a size in bytes equal or greater than this.
    pub flush_num_bytes: u64,

    /// Flush if the accumulated payload has a number of rows equal or greater than this.
    pub flush_num_rows: u64,

    /// Size of the internal channel of [`DataTableBatcherCmd`]s.
    pub max_commands_in_flight: Option<u64>,

    /// Size of the internal channel of [`DataTable`]s.
    pub max_tables_in_flight: Option<u64>,
}

impl Default for DataTableBatcherConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl DataTableBatcherConfig {
    /// Default configuration, applicable to most use cases.
    pub const DEFAULT: Self = Self {
        flush_duration: Duration::from_millis(50),
        flush_num_bytes: 1024 * 1024, // 1 MiB
        flush_num_rows: u64::MAX,
        max_commands_in_flight: None,
        max_tables_in_flight: None,
    };

    /// Always flushes.
    pub const ALWAYS: Self = Self {
        flush_duration: Duration::MAX,
        flush_num_bytes: 0,
        flush_num_rows: 0,
        max_commands_in_flight: None,
        max_tables_in_flight: None,
    };

    /// Never flushes unless manually told to.
    pub const NEVER: Self = Self {
        flush_duration: Duration::MAX,
        flush_num_bytes: u64::MAX,
        flush_num_rows: u64::MAX,
        max_commands_in_flight: None,
        max_tables_in_flight: None,
    };

    // TODO: why is this here tho..
    pub const ENV_FLUSH_DURATION: &str = "RERUN_FLUSH_DURATION_SECS";
    pub const ENV_FLUSH_NUM_BYTES: &str = "RERUN_FLUSH_NUM_BYTES";
    pub const ENV_FLUSH_NUM_ROWS: &str = "RERUN_FLUSH_NUM_ROWS";

    /// Creates a new `DataTableBatcherConfig` using the default values, optionally overridden
    /// through the environment.
    ///
    /// See [`Self::ENV_FLUSH_DURATION`], [`Self::ENV_FLUSH_NUM_BYTES`],
    /// [`Self::ENV_FLUSH_NUM_BYTES`].
    pub fn from_env() -> DataTableBatcherResult<Self> {
        let mut this = Self::default();

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_DURATION) {
            let flush_duration_secs: f64 =
                s.parse()
                    .map_err(|err| DataTableBatcherError::ParseConfig {
                        name: Self::ENV_FLUSH_DURATION,
                        value: s.clone(),
                        err: Box::new(err),
                    })?;

            this.flush_duration = Duration::from_secs_f64(flush_duration_secs);
        }

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_NUM_BYTES) {
            this.flush_num_bytes = s
                .parse()
                .map_err(|err| DataTableBatcherError::ParseConfig {
                    name: Self::ENV_FLUSH_NUM_BYTES,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
        }

        if let Ok(s) = std::env::var(Self::ENV_FLUSH_NUM_ROWS) {
            this.flush_num_rows = s
                .parse()
                .map_err(|err| DataTableBatcherError::ParseConfig {
                    name: Self::ENV_FLUSH_NUM_ROWS,
                    value: s.clone(),
                    err: Box::new(err),
                })?;
        }

        Ok(this)
    }
}

// Detect breaking changes in our environment variables.
#[test]
fn data_table_batcher_config() {
    std::env::set_var("RERUN_FLUSH_DURATION_SECS", "0.3");
    std::env::set_var("RERUN_FLUSH_NUM_BYTES", "42");
    std::env::set_var("RERUN_FLUSH_NUM_ROWS", "666");

    let config = DataTableBatcherConfig::from_env().unwrap();

    let expected = DataTableBatcherConfig {
        flush_duration: Duration::from_millis(300),
        flush_num_bytes: 42,
        flush_num_rows: 666,
        ..Default::default()
    };

    assert_eq!(expected, config);
}

// ---

// TODO: def make this a higher-level API

// TODO: this needs a way to not block on shutdown.. a simple boolean should do the trick?

// TODO: clean the shutdown semantantics I hate it

// TODO: we absolutely need to document shutdown semantics, this is a mess
// it's fairly possible that we don't even want drop logic in this instance?

// TODO: doc:
// - clone
// - multithreading
// - shutdown
// - flush
// - drop
// - cheap to clone
#[derive(Clone)]
pub struct DataTableBatcher {
    inner: Arc<DataTableBatcherInner>,
}

// NOTE: The receiving end of the command stream as well as the sending end of the table stream are
// owned solely by the batching thread.
struct DataTableBatcherInner {
    tx_cmds: Sender<Command>,
    rx_tables: Receiver<(RecordingId, DataTable)>,
    cmds_to_tables_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for DataTableBatcherInner {
    fn drop(&mut self) {
        self.shutdown().ok();
        if let Some(handle) = self.cmds_to_tables_handle.take() {
            handle.join().ok();
        }
    }
}

// TODO: explain multithread behavior
// TODO: why is this clone?
#[derive(Debug, Clone)]
enum Command {
    AppendRow(RecordingId, DataRow),
    Flush(Sender<()>),
    Shutdown,
}

impl Command {
    fn flush() -> (Self, Receiver<()>) {
        let (tx, rx) = crossbeam::channel::bounded(0); // oneshot
        (Self::Flush(tx), rx)
    }
}

// TODO: make it record dedicated

impl DataTableBatcher {
    // TODO
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
            rx_tables,
            cmds_to_tables_handle: Some(cmds_to_tables_handle),
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    // --- Send commands ---

    // TODO
    // - what are the guarantees from the caller's pov?
    #[inline]
    pub fn append_row(
        &self,
        rid: RecordingId,
        row: DataRow,
    ) -> Result<(), SendError<(RecordingId, DataRow)>> {
        self.inner.append_row(rid, row)
    }

    // TODO
    #[inline]
    pub fn flush_and_forget(&self) -> Result<(), SendError<()>> {
        self.inner.flush_and_forget()
    }

    // TODO
    // - what are the guarantees from the caller's pov?
    // TODO: explain when and why this is safe from a concurrency perspective
    #[inline]
    pub fn flush_blocking(&self) -> Result<(), SendError<()>> {
        self.inner.flush_blocking()
    }

    #[inline]
    pub fn shutdown(&self) -> Result<(), SendError<()>> {
        self.inner.shutdown()
    }

    // --- Subscribe to tables ---

    pub fn tables(&self) -> Receiver<(RecordingId, DataTable)> {
        self.inner.rx_tables.clone()
    }
}

impl DataTableBatcherInner {
    fn append_row(
        &self,
        rid: RecordingId,
        row: DataRow,
    ) -> Result<(), SendError<(RecordingId, DataRow)>> {
        self.send_cmd(Command::AppendRow(rid, row))
            .map_err(|err| match err.0 {
                Command::AppendRow(rid, row) => SendError((rid, row)),
                _ => unreachable!(),
            })
    }

    fn flush_and_forget(&self) -> Result<(), SendError<()>> {
        let (flush_cmd, _) = Command::flush();
        self.send_cmd(flush_cmd).map_err(|_err| SendError(()))?;
        Ok(())
    }

    fn flush_blocking(&self) -> Result<(), SendError<()>> {
        let (flush_cmd, oneshot) = Command::flush();
        self.send_cmd(flush_cmd).map_err(|_err| SendError(()))?;
        oneshot.recv().ok();
        Ok(())
    }

    fn shutdown(&self) -> Result<(), SendError<()>> {
        self.flush_and_forget()?;
        self.send_cmd(Command::Shutdown)
            .map_err(|_err| SendError(()))?;
        Ok(())
    }

    fn send_cmd(&self, cmd: Command) -> Result<(), SendError<Command>> {
        self.tx_cmds.send(cmd)
    }
}

#[allow(clippy::needless_pass_by_value)]
fn batching_thread(
    config: DataTableBatcherConfig,
    rx_cmd: Receiver<Command>,
    tx_table: Sender<(RecordingId, DataTable)>,
) {
    let rx_tick = crossbeam::channel::tick(config.flush_duration);

    struct Accumulator {
        latest: Instant,
        pending_tables: IntMap<RecordingId, Vec<DataRow>>,
        pending_num_rows: u64,
        pending_num_bytes: u64,
    }
    impl Accumulator {
        fn reset(&mut self) {
            self.latest = Instant::now();
            self.pending_tables.clear();
            self.pending_num_rows = 0;
            self.pending_num_bytes = 0;
        }
    }

    let mut acc = Accumulator {
        latest: Instant::now(),
        pending_tables: Default::default(),
        pending_num_rows: Default::default(),
        pending_num_bytes: Default::default(),
    };

    use crossbeam::select;
    loop {
        select! {
            recv(rx_cmd) -> cmd => {
                let Ok(cmd) = cmd else {
                    // All command senders are gone, which can only happen if the
                    // `DataTableBatcher` itself has been dropped.
                    do_flush_all(&mut acc, &tx_table, "all_dropped");
                    break;
                };

                match cmd {
                    Command::AppendRow(rid, row) => {
                        if do_append_row(&config, &mut acc, rid, row) {
                            do_flush_all(&mut acc, &tx_table, "bytes|rows");
                            acc.reset();
                        }
                    },
                    Command::Flush(oneshot) => {
                        do_flush_all(&mut acc, &tx_table, "manual");
                        acc.reset();
                        drop(oneshot); // signals the oneshot
                    },
                    Command::Shutdown => break,
                };
            },
            recv(rx_tick) -> _ => {
                do_flush_all(&mut acc, &tx_table, "duration");
                acc.reset();
            },
        };

        // NOTE: The receiving end of the command stream as well as the sending end of the table
        // stream are owned solely by this thread.
        // Past this point, all command writes and all table reads will return `ErrDisconnected`.
    }

    fn do_append_row(
        config: &DataTableBatcherConfig,
        acc: &mut Accumulator,
        rid: RecordingId,
        mut row: DataRow,
    ) -> bool {
        // TODO: how do we document that? that's a bit weird but eh
        // TODO: if we're doing this here, it really is a massive waste not to send it over the
        // wire... link issue
        row.compute_all_size_bytes();

        acc.pending_num_rows += 1;
        acc.pending_num_bytes += row.total_size_bytes();

        let pending_rows = acc.pending_tables.entry(rid).or_default();
        pending_rows.push(row);

        acc.pending_num_rows >= config.flush_num_rows
            || acc.pending_num_bytes >= config.flush_num_bytes
    }

    fn do_flush_all(
        acc: &mut Accumulator,
        tx_table: &Sender<(RecordingId, DataTable)>,
        reason: &str,
    ) {
        re_log::trace!(reason, "flushing tables");
        for (rid, mut pending_rows) in acc.pending_tables.drain() {
            do_flush(rid, &mut pending_rows, tx_table);
        }
    }

    fn do_flush(
        rid: RecordingId,
        rows: &mut Vec<DataRow>,
        tx_table: &Sender<(RecordingId, DataTable)>,
    ) {
        if rows.is_empty() {
            return;
        }

        let table = DataTable::from_rows(TableId::random(), rows.drain(..));
        // TODO: sort? optional? it's nice to offer sorting in another thread for free!
        // table.sort();

        // NOTE: This can only fail if all receivers have been dropped, which simply cannot happen
        // as long the batching thread is alive.
        tx_table.send((rid, table)).ok();
    }
}

// ---

#[cfg(test)]
mod tests {
    use super::*;

    use crossbeam::{channel::TryRecvError, select};
    use itertools::Itertools as _;

    use crate::{DataRow, RowId, SizeBytes, TimePoint, Timeline};

    #[test]
    fn manual_trigger() {
        let rid = RecordingId::ZERO;
        let batcher = DataTableBatcher::new(DataTableBatcherConfig::NEVER).unwrap();
        let tables = batcher.tables();

        let expected = create_table();

        for _ in 0..3 {
            assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

            for row in expected.to_rows() {
                batcher.append_row(rid, row).unwrap();
            }

            assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

            batcher.flush_blocking().unwrap();

            {
                let batch = tables.recv().unwrap();
                // NOTE: Override the resulting table's ID so they can be compared.
                let (_rid, mut table) = batch;
                table.table_id = expected.table_id;

                similar_asserts::assert_eq!(expected, table);
            }

            assert_eq!(Err(TryRecvError::Empty), tables.try_recv());
        }

        drop(batcher);

        assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
    }

    #[test]
    fn shutdown_trigger() {
        let rid = RecordingId::ZERO;
        let batcher = DataTableBatcher::new(DataTableBatcherConfig::NEVER).unwrap();
        let tables = batcher.tables();

        let rows = create_table().to_rows().collect_vec();

        for _ in 0..3 {
            assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

            for row in rows.clone() {
                batcher.append_row(rid, row).unwrap();
            }

            assert_eq!(Err(TryRecvError::Empty), tables.try_recv());
        }

        drop(batcher);

        let expected = DataTable::from_rows(
            TableId::ZERO,
            std::iter::repeat_with(|| rows.clone()).take(3).flatten(),
        );

        select! {
                recv(tables) -> batch => {
                // NOTE: Override the resulting table's ID so they can be compared.
                let (_rid, mut table) = batch.unwrap();
                table.table_id = expected.table_id;

                similar_asserts::assert_eq!(expected, table);
            }
            default(Duration::from_millis(50)) => {
                panic!("output channel never yielded any table");
            }
        }

        assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
    }

    #[test]
    fn num_bytes_trigger() {
        let mut table = create_table();
        table.compute_all_size_bytes();

        let rows = table.to_rows().collect_vec();
        let flush_duration = std::time::Duration::from_millis(50);
        let flush_num_bytes = rows
            .iter()
            .take(rows.len() - 1)
            .map(|row| row.total_size_bytes())
            .sum::<u64>();

        let rid = RecordingId::ZERO;
        let batcher = DataTableBatcher::new(DataTableBatcherConfig {
            flush_num_bytes,
            flush_duration,
            ..DataTableBatcherConfig::NEVER
        })
        .unwrap();
        let tables = batcher.tables();

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        for row in table.to_rows() {
            batcher.append_row(rid, row).unwrap();
        }

        // Expect all rows except for the last one (num_bytes trigger).
        select! {
                recv(tables) -> batch => {
                let (_rid, table) = batch.unwrap();
                let expected = DataTable::from_rows(
                    table.table_id,
                    rows.clone().into_iter().take(rows.len() - 1),
                );
                similar_asserts::assert_eq!(expected, table);
            }
            default(flush_duration) => {
                panic!("output channel never yielded any table");
            }
        }

        // Expect just the last row (duration trigger).
        select! {
                recv(tables) -> batch => {
                let (_rid, table) = batch.unwrap();
                let expected = DataTable::from_rows(
                    table.table_id,
                    rows.last().cloned(),
                );
                similar_asserts::assert_eq!(expected, table);
            }
            default(flush_duration * 2) => {
                panic!("output channel never yielded any table");
            }
        }

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        drop(batcher);

        assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
    }

    #[test]
    fn num_rows_trigger() {
        let table = create_table();

        let rows = table.to_rows().collect_vec();
        let flush_duration = std::time::Duration::from_millis(50);
        let flush_num_rows = rows.len() as u64 - 1;

        let rid = RecordingId::ZERO;
        let batcher = DataTableBatcher::new(DataTableBatcherConfig {
            flush_num_rows,
            flush_duration,
            ..DataTableBatcherConfig::NEVER
        })
        .unwrap();
        let tables = batcher.tables();

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        for row in table.to_rows() {
            batcher.append_row(rid, row).unwrap();
        }

        // Expect all rows except for the last one.
        select! {
                recv(tables) -> batch => {
                let (_rid, table) = batch.unwrap();
                let expected = DataTable::from_rows(
                    table.table_id,
                    rows.clone().into_iter().take(rows.len() - 1),
                );
                similar_asserts::assert_eq!(expected, table);
            }
            default(flush_duration) => {
                panic!("output channel never yielded any table");
            }
        }

        // Expect just the last row.
        select! {
                recv(tables) -> batch => {
                let (_rid, table) = batch.unwrap();
                let expected = DataTable::from_rows(
                    table.table_id,
                    rows.last().cloned(),
                );
                similar_asserts::assert_eq!(expected, table);
            }
            default(flush_duration * 2) => {
                panic!("output channel never yielded any table");
            }
        }

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        drop(batcher);

        assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
    }

    #[test]
    fn duration_trigger() {
        let table = create_table();
        let rows = table.to_rows().collect_vec();

        let flush_duration = Duration::from_millis(50);

        let rid = RecordingId::ZERO;
        let batcher = DataTableBatcher::new(DataTableBatcherConfig {
            flush_duration,
            ..DataTableBatcherConfig::NEVER
        })
        .unwrap();
        let tables = batcher.tables();

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        _ = std::thread::Builder::new().spawn({
            let mut rows = rows.clone();
            let batcher = batcher.clone();
            move || {
                for row in rows.drain(..rows.len() - 1) {
                    batcher.append_row(rid, row).unwrap();
                }

                std::thread::sleep(flush_duration * 2);

                let row = rows.last().cloned().unwrap();
                batcher.append_row(rid, row).unwrap();
            }
        });

        // Expect all rows except for the last one.
        select! {
                recv(tables) -> batch => {
                let (_rid, table) = batch.unwrap();
                let expected = DataTable::from_rows(
                    table.table_id,
                    rows.clone().into_iter().take(rows.len() - 1),
                );
                similar_asserts::assert_eq!(expected, table);
            }
            default(flush_duration * 2) => {
                panic!("output channel never yielded any table");
            }
        }

        // Expect just the last row.
        select! {
                recv(tables) -> batch => {
                let (_rid, table) = batch.unwrap();
                let expected = DataTable::from_rows(
                    table.table_id,
                    rows.last().cloned(),
                );
                similar_asserts::assert_eq!(expected, table);
            }
            default(flush_duration * 4) => {
                panic!("output channel never yielded any table");
            }
        }

        assert_eq!(Err(TryRecvError::Empty), tables.try_recv());

        drop(batcher);

        assert_eq!(Err(TryRecvError::Disconnected), tables.try_recv());
    }

    fn create_table() -> DataTable {
        use crate::{
            component_types::{ColorRGBA, Label, Point2D},
            Time,
        };

        let timepoint = |frame_nr: i64| {
            TimePoint::from([
                (Timeline::new_temporal("log_time"), Time::now().into()),
                (Timeline::new_sequence("frame_nr"), frame_nr.into()),
            ])
        };

        let row0 = {
            let num_instances = 2;
            let points: &[Point2D] = &[[10.0, 10.0].into(), [20.0, 20.0].into()];
            let colors: &[_] = &[ColorRGBA::from_rgb(128, 128, 128)];
            let labels: &[Label] = &[];

            DataRow::from_cells3(
                RowId::random(),
                "a",
                timepoint(1),
                num_instances,
                (points, colors, labels),
            )
        };

        let row1 = {
            let num_instances = 0;
            let colors: &[ColorRGBA] = &[];

            DataRow::from_cells1(RowId::random(), "b", timepoint(1), num_instances, colors)
        };

        let row2 = {
            let num_instances = 1;
            let colors: &[_] = &[ColorRGBA::from_rgb(255, 255, 255)];
            let labels: &[_] = &[Label("hey".into())];

            DataRow::from_cells2(
                RowId::random(),
                "c",
                timepoint(2),
                num_instances,
                (colors, labels),
            )
        };

        DataTable::from_rows(TableId::ZERO, [row0, row1, row2])
    }
}
