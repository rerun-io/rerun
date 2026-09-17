//! Imports `.puffin` profiler captures as state changes.
//!
//! See <https://github.com/EmbarkStudios/puffin>.
//!
//! Every profiled thread becomes an entity, and every call depth of that thread becomes one lane
//! of that entity's state array: the lane holds the scope that is active at that depth.

use std::collections::BTreeMap;

use arrow::array::{ListBuilder, StringBuilder};
use crossbeam::channel::Sender;
use puffin::{FrameData, NanoSecond, ScopeCollection, Stream};

use re_chunk::{Chunk, ChunkId, TimeColumn};
use re_log_types::{EntityPath, TimeType};
use re_sdk_types::archetypes::StateChange;

use crate::{ImportedData, Importer, ImporterError, ImporterSettings};

const PUFFIN_IMPORTER_NAME: &str = "rerun.importers.Puffin";

/// The first bytes of every `.puffin` file.
const PUFFIN_MAGIC: &[u8; 4] = b"PUF0";

/// Carries the wall-clock time of each scope transition.
const TIME_TIMELINE: &str = "time";

/// Carries the index of the puffin frame a transition belongs to.
const FRAME_TIMELINE: &str = "frame_nr";

/// How many state rows one chunk holds at most.
const MAX_ROWS_PER_CHUNK: usize = 8192;

/// An [`Importer`] for `.puffin` profiler captures.
///
/// The scopes of one thread are laid out as one lane per call depth, so the state timeline view
/// shows the call stack of each thread over time.
pub struct PuffinImporter;

impl Importer for PuffinImporter {
    fn name(&self) -> crate::ImporterName {
        PUFFIN_IMPORTER_NAME.into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_from_path(
        &self,
        settings: &ImporterSettings,
        path: std::path::PathBuf,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        if !path.is_file() || crate::extension(&path) != "puffin" {
            return Err(ImporterError::Incompatible(path));
        }

        re_tracing::profile_function!(path.display().to_string());

        let contents = {
            re_tracing::profile_scope!("fs::read");
            std::fs::read(&path)?
        };

        self.import_from_file_contents(settings, path, contents.into(), tx)
    }

    fn import_from_file_contents(
        &self,
        settings: &ImporterSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        if crate::extension(&filepath) != "puffin" || !contents.starts_with(PUFFIN_MAGIC) {
            return Err(ImporterError::Incompatible(filepath));
        }

        re_tracing::profile_function!(filepath.display().to_string());

        let store_id = settings.opened_store_id_or_recommended();
        let timelines = read_timelines(&contents).map_err(|err| err.with_path(&filepath))?;

        let mut num_sent = 0;
        'threads: for (thread_name, timeline) in timelines {
            let entity_path = entity_path_for_thread(&thread_name, settings);
            let chunks = chunks_for_thread(&entity_path, &timeline, settings)
                .map_err(|err| err.with_path(&filepath))?;

            for chunk in chunks {
                let data = ImportedData::Chunk(self.name(), store_id.clone(), chunk);
                if re_quota_channel::send_crossbeam(&tx, data).is_err() {
                    break 'threads;
                }
                num_sent += 1;
            }
        }

        if num_sent == 0 {
            re_log::warn!(
                "No profiler scopes found.\nFile path: {}",
                filepath.display()
            );
        }

        Ok(())
    }
}

/// Reads every frame of the capture and sorts the scopes it holds by thread name.
///
/// Puffin tells threads apart by name and start time, so a re-spawned thread is reported as a new
/// one. Keying on the name gives each name one entity with a fixed number of lanes.
fn read_timelines(contents: &[u8]) -> Result<BTreeMap<String, ThreadTimeline>, ImporterError> {
    re_tracing::profile_function!();

    let mut read = &contents[PUFFIN_MAGIC.len()..];

    // Scope details arrive as a delta, so a scope recorded in a late frame may have been named in
    // an early one.
    let mut scopes = ScopeCollection::default();
    let mut timelines: BTreeMap<String, ThreadTimeline> = BTreeMap::new();

    while let Some(frame) = FrameData::read_next(&mut read).map_err(ImporterError::Other)? {
        for details in &frame.scope_delta {
            scopes.insert(details.clone());
        }

        let frame_index = frame.frame_index();
        let unpacked = frame.unpacked().map_err(ImporterError::Other)?;

        for (thread, stream_info) in &unpacked.thread_streams {
            let timeline = timelines.entry(thread.name.clone()).or_default();
            collect_scopes(&stream_info.stream, 0, 0, frame_index, &scopes, timeline)?;
        }
    }

    Ok(timelines)
}

/// Walks the scopes at one call depth and all of their children, recording their transitions.
fn collect_scopes(
    stream: &Stream,
    offset: u64,
    depth: usize,
    frame_index: u64,
    scopes: &ScopeCollection,
    timeline: &mut ThreadTimeline,
) -> Result<(), ImporterError> {
    let reader = puffin::Reader::with_offset(stream, offset).map_err(|err| stream_error(&err))?;

    for scope in reader {
        let scope = scope.map_err(|err| stream_error(&err))?;

        timeline.num_lanes = timeline.num_lanes.max(depth + 1);

        let label = scope_label(&scope, scopes);
        timeline
            .transition_at(scope.record.start_ns, frame_index)
            .started
            .push((depth, label));

        timeline
            .transition_at(scope.record.stop_ns(), frame_index)
            .ended
            .push(depth);

        collect_scopes(
            stream,
            scope.child_begin_position,
            depth + 1,
            frame_index,
            scopes,
            timeline,
        )?;
    }

    Ok(())
}

/// [`puffin::Error`] implements neither [`std::fmt::Display`] nor [`std::error::Error`], so each
/// variant is spelled out here.
fn stream_error(err: &puffin::Error) -> ImporterError {
    let reason = match err {
        puffin::Error::PrematureEnd => "the scope stream ended early",
        puffin::Error::InvalidStream => "the scope stream is malformed",
        puffin::Error::ScopeNeverEnded => "a scope was never closed",
        puffin::Error::InvalidOffset => "a scope points outside of its stream",
        puffin::Error::Empty => "the scope stream is empty",
    };

    ImporterError::Other(anyhow::anyhow!(
        "Failed to parse puffin profiler scopes: {reason}."
    ))
}

/// What one thread did over the whole capture.
#[derive(Default)]
struct ThreadTimeline {
    /// The lane changes of the thread, keyed by the time they happen at.
    transitions: BTreeMap<NanoSecond, Transition>,

    /// One lane per call depth the thread reached.
    num_lanes: usize,
}

impl ThreadTimeline {
    fn transition_at(&mut self, time_ns: NanoSecond, frame_index: u64) -> &mut Transition {
        let transition = self.transitions.entry(time_ns).or_default();
        transition.frame_index = frame_index;
        transition
    }
}

/// The lane changes that happen at a single point in time.
#[derive(Default)]
struct Transition {
    /// The lanes whose scope ends here.
    ended: Vec<usize>,

    /// The lanes whose scope starts here, with the label to show.
    started: Vec<(usize, String)>,

    /// The frame this time falls into.
    frame_index: u64,
}

/// The label shown for a scope: its name, followed by the data the caller passed along.
fn scope_label(scope: &puffin::Scope<'_>, scopes: &ScopeCollection) -> String {
    let name = scopes
        .fetch_by_id(&scope.id)
        .map_or_else(|| format!("scope {}", scope.id.0), |d| d.name().to_string());

    if scope.record.data.is_empty() {
        name
    } else {
        format!("{name}: {}", scope.record.data)
    }
}

/// Replays the transitions of one thread into state rows, one row per point in time.
fn chunks_for_thread(
    entity_path: &EntityPath,
    timeline: &ThreadTimeline,
    settings: &ImporterSettings,
) -> Result<Vec<Chunk>, ImporterError> {
    re_tracing::profile_function!();

    let mut chunks = Vec::new();
    let mut columns = StateColumns::default();

    // The scope active at each call depth, carried over from the previous row.
    let mut stack: Vec<Option<String>> = vec![None; timeline.num_lanes];

    for (time_ns, transition) in &timeline.transitions {
        // Ends are applied first, so a scope that starts where the previous one ended takes over
        // the lane.
        for &depth in &transition.ended {
            stack[depth] = None;
        }
        for (depth, label) in &transition.started {
            stack[*depth] = Some(label.clone());
        }

        columns.push_row(*time_ns, transition.frame_index, &stack, settings);

        if MAX_ROWS_PER_CHUNK <= columns.num_rows() {
            chunks.push(columns.build_chunk(entity_path, settings)?);
        }
    }

    if 0 < columns.num_rows() {
        chunks.push(columns.build_chunk(entity_path, settings)?);
    }

    Ok(chunks)
}

/// The columns of one chunk.
#[derive(Default)]
struct StateColumns {
    /// The time of each row, with `timestamp_offset_ns` applied.
    times_ns: Vec<i64>,

    /// The puffin frame each row belongs to.
    frame_indices: Vec<i64>,

    /// One lane label per call depth, per row.
    states: ListBuilder<StringBuilder>,
}

impl StateColumns {
    fn num_rows(&self) -> usize {
        self.times_ns.len()
    }

    fn push_row(
        &mut self,
        time_ns: NanoSecond,
        frame_index: u64,
        stack: &[Option<String>],
        settings: &ImporterSettings,
    ) {
        self.times_ns
            .push(time_ns.saturating_add(settings.timestamp_offset_ns.unwrap_or(0)));
        self.frame_indices
            .push(i64::try_from(frame_index).unwrap_or(i64::MAX));

        for label in stack {
            self.states.values().append_option(label.as_deref());
        }
        self.states.append(true);
    }

    /// Turns the rows collected so far into a chunk, leaving the columns empty.
    fn build_chunk(
        &mut self,
        entity_path: &EntityPath,
        settings: &ImporterSettings,
    ) -> Result<Chunk, ImporterError> {
        let times_ns = std::mem::take(&mut self.times_ns);
        let frame_indices = std::mem::take(&mut self.frame_indices);

        // Puffin records nanoseconds since the unix epoch.
        let time_column = match settings.timeline_type {
            TimeType::DurationNs => TimeColumn::new_duration_nanos(TIME_TIMELINE, times_ns),
            TimeType::Sequence | TimeType::TimestampNs => {
                TimeColumn::new_timestamp_nanos_since_epoch(TIME_TIMELINE, times_ns)
            }
        };

        let timelines = [
            (TIME_TIMELINE.into(), time_column),
            (
                FRAME_TIMELINE.into(),
                TimeColumn::new_sequence(FRAME_TIMELINE, frame_indices),
            ),
        ]
        .into_iter()
        .collect();

        let components =
            std::iter::once((StateChange::descriptor_state(), self.states.finish())).collect();

        Chunk::from_auto_row_ids(ChunkId::new(), entity_path.clone(), timelines, components)
            .map_err(Into::into)
    }
}

fn entity_path_for_thread(thread_name: &str, settings: &ImporterSettings) -> EntityPath {
    let path = EntityPath::from_single_string(thread_name.to_owned());
    settings
        .entity_path_prefix
        .as_ref()
        .map_or_else(|| path.clone(), |prefix| prefix.clone() / path.clone())
}
