use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

use arrow2::array::{
    Array as ArrowArray, ListArray as ArrowListArray, PrimitiveArray as ArrowPrimitiveArray,
    StructArray as ArrowStructArray,
};

use itertools::{izip, Itertools};
use re_log_types::{EntityPath, ResolvedTimeRange, Time, TimeInt, TimePoint, Timeline};
use re_types_core::{ComponentName, Loggable, LoggableBatch, SerializationError, SizeBytes};

use crate::{ChunkId, RowId};

// ---

/// Errors that can occur when creating/manipulating a [`Chunk`]s, directly or indirectly through
/// the use of a [`crate::ChunkBatcher`].
#[derive(thiserror::Error, Debug)]
pub enum ChunkError {
    #[error("Detected malformed Chunk: {reason}")]
    Malformed { reason: String },

    #[error(transparent)]
    Serialization(#[from] SerializationError),
}

pub type ChunkResult<T> = Result<T, ChunkError>;

// ---

/// Dense arrow-based storage of N rows of multi-component multi-temporal data for a specific entity.
///
/// This is our core datastructure for logging, storing, querying and transporting data around.
///
/// The chunk as a whole is always ascendingly sorted by [`RowId`] before it gets manipulated in any way.
/// Its time columns might or might not be ascendingly sorted, depending on how the data was logged.
///
/// This is the in-memory representation of a chunk, optimized for efficient manipulation of the
/// data within. For transport, see [`crate::TransportChunk`] instead.
#[derive(Debug)]
pub struct Chunk {
    pub(crate) id: ChunkId,

    pub(crate) entity_path: EntityPath,

    /// The heap size of this chunk in bytes.
    ///
    /// Must be cached as it is very costly to compute, and needs to be computed repeatedly on the
    /// hot path (e.g. during garbage collection).
    pub(crate) heap_size_bytes: AtomicU64,

    /// Is the chunk as a whole sorted by [`RowId`]?
    pub(crate) is_sorted: bool,

    /// The respective [`RowId`]s for each row of data.
    pub(crate) row_ids: ArrowStructArray,

    /// The time columns.
    ///
    /// Each column must be the same length as `row_ids`.
    ///
    /// Empty if this is a static chunk.
    pub(crate) timelines: BTreeMap<Timeline, ChunkTimeline>,

    /// A sparse `ListArray` for each component.
    ///
    /// Each `ListArray` must be the same length as `row_ids`.
    ///
    /// Sparse so that we can e.g. log a `Position` at one timestamp but not a `Color`.
    //
    // TODO(#6576): support non-list based columns?
    pub(crate) components: BTreeMap<ComponentName, ArrowListArray<i32>>,
}

impl PartialEq for Chunk {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self {
            id,
            entity_path,
            heap_size_bytes: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        *id == other.id
            && *entity_path == other.entity_path
            && *is_sorted == other.is_sorted
            && *row_ids == other.row_ids
            && *timelines == other.timelines
            && *components == other.components
    }
}

impl Chunk {
    /// Returns `true` is two [`Chunk`]s are similar, although not byte-for-byte equal.
    ///
    /// In particular, this ignores chunks and row IDs, as well as temporal timestamps.
    ///
    /// Useful for tests.
    pub fn are_similar(lhs: &Self, rhs: &Self) -> bool {
        let Self {
            id: _,
            entity_path,
            heap_size_bytes: _,
            is_sorted: _,
            row_ids: _,
            timelines,
            components,
        } = lhs;

        *entity_path == rhs.entity_path
            && timelines.keys().collect_vec() == rhs.timelines.keys().collect_vec()
            && {
                let timelines: BTreeMap<_, _> = timelines
                    .iter()
                    .filter(|(timeline, _time_chunk)| {
                        timeline.typ() != re_log_types::TimeType::Time
                    })
                    .collect();
                let rhs_timelines: BTreeMap<_, _> = rhs
                    .timelines
                    .iter()
                    .filter(|(timeline, _time_chunk)| {
                        timeline.typ() != re_log_types::TimeType::Time
                    })
                    .collect();
                timelines == rhs_timelines
            }
            && *components == rhs.components
    }
}

impl Clone for Chunk {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            entity_path: self.entity_path.clone(),
            heap_size_bytes: AtomicU64::new(self.heap_size_bytes.load(Ordering::Relaxed)),
            is_sorted: self.is_sorted,
            row_ids: self.row_ids.clone(),
            timelines: self.timelines.clone(),
            components: self.components.clone(),
        }
    }
}

impl Chunk {
    /// Clones the chunk and assign new IDs to the resulting chunk and its rows.
    ///
    /// `first_row_id` will become the [`RowId`] of the first row in the duplicated chunk.
    /// Each row after that will be monotonically increasing.
    #[inline]
    pub fn clone_as(&self, id: ChunkId, first_row_id: RowId) -> Self {
        let row_ids = std::iter::from_fn({
            let mut row_id = first_row_id;
            move || {
                let yielded = row_id;
                row_id = row_id.next();
                Some(yielded)
            }
        })
        .take(self.row_ids.len())
        .collect_vec();

        #[allow(clippy::unwrap_used)]
        let row_ids = <RowId as Loggable>::to_arrow(&row_ids)
            // Unwrap: native RowIds cannot fail to serialize.
            .unwrap()
            .as_any()
            .downcast_ref::<ArrowStructArray>()
            // Unwrap: RowId schema is known in advance to be a struct array -- always.
            .unwrap()
            .clone();

        Self {
            id,
            row_ids,
            ..self.clone()
        }
    }

    /// Clones the chunk into a new chunk without any time data.
    #[inline]
    pub fn into_static(mut self) -> Self {
        self.timelines.clear();
        self
    }

    /// Computes the time range covered by each individual component column on each timeline.
    ///
    /// This is different from the time range covered by the [`Chunk`] as a whole because component
    /// columns are potentially sparse.
    ///
    /// This is crucial for indexing and queries to work properly.
    //
    // TODO(cmc): This needs to be stored in chunk metadata and transported across IPC.
    #[inline]
    pub fn time_range_per_component(
        &self,
    ) -> BTreeMap<Timeline, BTreeMap<ComponentName, ResolvedTimeRange>> {
        re_tracing::profile_function!();

        self.timelines
            .iter()
            .map(|(&timeline, time_chunk)| {
                (
                    timeline,
                    time_chunk.time_range_per_component(&self.components),
                )
            })
            .collect()
    }

    /// Computes the `RowId` range covered by each individual component column on each timeline.
    ///
    /// This is different from the `RowId` range covered by the [`Chunk`] as a whole because component
    /// columns are potentially sparse.
    ///
    /// This is crucial for indexing and queries to work properly.
    //
    // TODO(cmc): This needs to be stored in chunk metadata and transported across IPC.
    pub fn row_id_range_per_component(&self) -> BTreeMap<ComponentName, (RowId, RowId)> {
        re_tracing::profile_function!();

        let row_ids = self.row_ids().collect_vec();

        if self.is_sorted() {
            self.components
                .iter()
                .filter_map(|(component_name, list_array)| {
                    let mut row_id_min = None;
                    let mut row_id_max = None;

                    for (i, &row_id) in row_ids.iter().enumerate() {
                        if list_array.is_valid(i) {
                            row_id_min = Some(row_id);
                        }
                    }
                    for (i, &row_id) in row_ids.iter().enumerate().rev() {
                        if list_array.is_valid(i) {
                            row_id_max = Some(row_id);
                        }
                    }

                    Some((*component_name, (row_id_min?, row_id_max?)))
                })
                .collect()
        } else {
            self.components
                .iter()
                .filter_map(|(component_name, list_array)| {
                    let mut row_id_min = Some(RowId::MAX);
                    let mut row_id_max = Some(RowId::ZERO);

                    for (i, &row_id) in row_ids.iter().enumerate() {
                        if list_array.is_valid(i) && Some(row_id) > row_id_min {
                            row_id_min = Some(row_id);
                        }
                    }
                    for (i, &row_id) in row_ids.iter().enumerate().rev() {
                        if list_array.is_valid(i) && Some(row_id) < row_id_max {
                            row_id_max = Some(row_id);
                        }
                    }

                    Some((*component_name, (row_id_min?, row_id_max?)))
                })
                .collect()
        }
    }
}

// ---

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkTimeline {
    pub(crate) timeline: Timeline,

    /// Every single timestamp for this timeline.
    ///
    /// * This might or might not be sorted, depending on how the data was logged.
    /// * This is guaranteed to always be dense, because chunks are split anytime a timeline is
    ///   added or removed.
    /// * This cannot ever contain `TimeInt::STATIC`, since static data doesn't even have timelines.
    pub(crate) times: ArrowPrimitiveArray<i64>,

    /// Is [`Self::times`] sorted?
    ///
    /// This is completely independent of [`Chunk::is_sorted`]: a timeline doesn't necessarily
    /// follow the global [`RowId`]-based order, although it does in most cases (happy path).
    pub(crate) is_sorted: bool,

    /// The time range covered by [`Self::times`].
    ///
    /// Not necessarily contiguous! Just the min and max value found in [`Self::times`].
    pub(crate) time_range: ResolvedTimeRange,
}

impl Chunk {
    /// Creates a new [`Chunk`].
    ///
    /// This will fail if the passed in data is malformed in any way -- see [`Self::sanity_check`]
    /// for details.
    ///
    /// Iff you know for sure whether the data is already appropriately sorted or not, specify `is_sorted`.
    /// When left unspecified (`None`), it will be computed in O(n) time.
    ///
    /// For a row-oriented constructor, see [`Self::builder`].
    pub fn new(
        id: ChunkId,
        entity_path: EntityPath,
        is_sorted: Option<bool>,
        row_ids: ArrowStructArray,
        timelines: BTreeMap<Timeline, ChunkTimeline>,
        components: BTreeMap<ComponentName, ArrowListArray<i32>>,
    ) -> ChunkResult<Self> {
        let mut chunk = Self {
            id,
            entity_path,
            heap_size_bytes: AtomicU64::new(0),
            is_sorted: false,
            row_ids,
            timelines,
            components,
        };

        chunk.is_sorted = is_sorted.unwrap_or_else(|| chunk.is_sorted_uncached());

        chunk.sanity_check()?;

        Ok(chunk)
    }

    /// Creates a new [`Chunk`].
    ///
    /// This will fail if the passed in data is malformed in any way -- see [`Self::sanity_check`]
    /// for details.
    ///
    /// Iff you know for sure whether the data is already appropriately sorted or not, specify `is_sorted`.
    /// When left unspecified (`None`), it will be computed in O(n) time.
    ///
    /// For a row-oriented constructor, see [`Self::builder`].
    pub fn from_native_row_ids(
        id: ChunkId,
        entity_path: EntityPath,
        is_sorted: Option<bool>,
        row_ids: &[RowId],
        timelines: BTreeMap<Timeline, ChunkTimeline>,
        components: BTreeMap<ComponentName, ArrowListArray<i32>>,
    ) -> ChunkResult<Self> {
        let row_ids = row_ids
            .to_arrow()
            // NOTE: impossible, but better safe than sorry.
            .map_err(|err| ChunkError::Malformed {
                reason: format!("RowIds failed to serialize: {err}"),
            })?
            .as_any()
            .downcast_ref::<ArrowStructArray>()
            // NOTE: impossible, but better safe than sorry.
            .ok_or_else(|| ChunkError::Malformed {
                reason: "RowIds failed to downcast".to_owned(),
            })?
            .clone();

        Self::new(id, entity_path, is_sorted, row_ids, timelines, components)
    }

    /// Creates a new [`Chunk`].
    ///
    /// This will fail if the passed in data is malformed in any way -- see [`Self::sanity_check`]
    /// for details.
    ///
    /// The data is assumed to be sorted in `RowId`-order. Sequential `RowId`s will be generated for each
    /// row in the chunk.
    pub fn from_auto_row_ids(
        id: ChunkId,
        entity_path: EntityPath,
        timelines: BTreeMap<Timeline, ChunkTimeline>,
        components: BTreeMap<ComponentName, ArrowListArray<i32>>,
    ) -> ChunkResult<Self> {
        let count = components
            .iter()
            .next()
            .map_or(0, |(_, list_array)| list_array.len());

        let row_ids = std::iter::from_fn({
            let tuid: re_tuid::Tuid = *id;
            let mut row_id = RowId::from_tuid(tuid.next());
            move || {
                let yielded = row_id;
                row_id = row_id.next();
                Some(yielded)
            }
        })
        .take(count)
        .collect_vec();

        Self::from_native_row_ids(id, entity_path, Some(true), &row_ids, timelines, components)
    }

    /// Simple helper for [`Self::new`] for static data.
    ///
    /// For a row-oriented constructor, see [`Self::builder`].
    #[inline]
    pub fn new_static(
        id: ChunkId,
        entity_path: EntityPath,
        is_sorted: Option<bool>,
        row_ids: ArrowStructArray,
        components: BTreeMap<ComponentName, ArrowListArray<i32>>,
    ) -> ChunkResult<Self> {
        Self::new(
            id,
            entity_path,
            is_sorted,
            row_ids,
            Default::default(),
            components,
        )
    }

    #[inline]
    pub fn empty(id: ChunkId, entity_path: EntityPath) -> Self {
        Self {
            id,
            entity_path,
            heap_size_bytes: Default::default(),
            is_sorted: true,
            row_ids: ArrowStructArray::new_empty(RowId::arrow_datatype()),
            timelines: Default::default(),
            components: Default::default(),
        }
    }

    #[inline]
    pub fn add_timeline(&mut self, chunk_timeline: ChunkTimeline) -> ChunkResult<()> {
        self.timelines
            .insert(chunk_timeline.timeline, chunk_timeline);
        self.sanity_check()
    }
}

impl ChunkTimeline {
    /// Creates a new [`ChunkTimeline`].
    ///
    /// Iff you know for sure whether the data is already appropriately sorted or not, specify `is_sorted`.
    /// When left unspecified (`None`), it will be computed in O(n) time.
    ///
    /// For a row-oriented constructor, see [`Self::builder`].
    pub fn new(
        is_sorted: Option<bool>,
        timeline: Timeline,
        times: ArrowPrimitiveArray<i64>,
    ) -> Self {
        re_tracing::profile_function!(format!("{} times", times.len()));

        let times = times.to(timeline.datatype());
        let time_slice = times.values().as_slice();

        let is_sorted =
            is_sorted.unwrap_or_else(|| time_slice.windows(2).all(|times| times[0] <= times[1]));

        let time_range = if is_sorted {
            // NOTE: The 'or' in 'map_or' is never hit, but better safe than sorry.
            let min_time = time_slice
                .first()
                .copied()
                .map_or(TimeInt::MIN, TimeInt::new_temporal);
            let max_time = time_slice
                .last()
                .copied()
                .map_or(TimeInt::MAX, TimeInt::new_temporal);
            ResolvedTimeRange::new(min_time, max_time)
        } else {
            // NOTE: Do the iteration multiple times in a cache-friendly way rather than the opposite.
            // NOTE: The 'or' in 'unwrap_or' is never hit, but better safe than sorry.
            let min_time = time_slice
                .iter()
                .min()
                .copied()
                .map_or(TimeInt::MIN, TimeInt::new_temporal);
            let max_time = time_slice
                .iter()
                .max()
                .copied()
                .map_or(TimeInt::MAX, TimeInt::new_temporal);
            ResolvedTimeRange::new(min_time, max_time)
        };

        Self {
            timeline,
            times,
            is_sorted,
            time_range,
        }
    }

    /// Creates a new [`ChunkTimeline`] of sequence type.
    pub fn new_sequence(
        name: impl Into<re_log_types::TimelineName>,
        times: impl IntoIterator<Item = impl Into<i64>>,
    ) -> Self {
        let time_vec = times.into_iter().map(|t| {
            let t = t.into();
            TimeInt::try_from(t)
                .unwrap_or_else(|_| {
                    re_log::error!(
                illegal_value = t,
                new_value = TimeInt::MIN.as_i64(),
                "ChunkTimeline::new_sequence() called with illegal value - clamped to minimum legal value"
            );
                    TimeInt::MIN
                })
                .as_i64()
        }).collect();

        Self::new(
            None,
            Timeline::new_sequence(name.into()),
            ArrowPrimitiveArray::<i64>::from_vec(time_vec),
        )
    }

    /// Creates a new [`ChunkTimeline`] of sequence type.
    pub fn new_seconds(
        name: impl Into<re_log_types::TimelineName>,
        times: impl IntoIterator<Item = impl Into<f64>>,
    ) -> Self {
        let time_vec = times.into_iter().map(|t| {
            let t = t.into();
            let time = Time::from_seconds_since_epoch(t);
            TimeInt::try_from(time)
                .unwrap_or_else(|_| {
                    re_log::error!(
                illegal_value = t,
                new_value = TimeInt::MIN.as_i64(),
                "ChunkTimeline::new_seconds() called with illegal value - clamped to minimum legal value"
            );
                    TimeInt::MIN
                })
                .as_i64()
        }).collect();

        Self::new(
            None,
            Timeline::new_sequence(name.into()),
            ArrowPrimitiveArray::<i64>::from_vec(time_vec),
        )
    }

    /// Creates a new [`ChunkTimeline`] of nanoseconds type.
    pub fn new_nanos(
        name: impl Into<re_log_types::TimelineName>,
        times: impl IntoIterator<Item = impl Into<i64>>,
    ) -> Self {
        let time_vec = times.into_iter().map(|t| {
            let t = t.into();
            let time = Time::from_ns_since_epoch(t);
            TimeInt::try_from(time)
                .unwrap_or_else(|_| {
                    re_log::error!(
                illegal_value = t,
                new_value = TimeInt::MIN.as_i64(),
                "ChunkTimeline::new_nanos() called with illegal value - clamped to minimum legal value"
            );
                    TimeInt::MIN
                })
                .as_i64()
        }).collect();

        Self::new(
            None,
            Timeline::new_sequence(name.into()),
            ArrowPrimitiveArray::<i64>::from_vec(time_vec),
        )
    }
}

// ---

impl Chunk {
    #[inline]
    pub fn id(&self) -> ChunkId {
        self.id
    }

    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }

    /// How many columns in total? Includes control, time, and component columns.
    #[inline]
    pub fn num_columns(&self) -> usize {
        let Self {
            id: _,
            entity_path: _, // not an actual column
            heap_size_bytes: _,
            is_sorted: _,
            row_ids: _,
            timelines,
            components,
        } = self;

        1 /* row_ids */ + timelines.len() + components.len()
    }

    #[inline]
    pub fn num_controls(&self) -> usize {
        _ = self;
        1 /* row_ids */
    }

    #[inline]
    pub fn num_timelines(&self) -> usize {
        self.timelines.len()
    }

    #[inline]
    pub fn num_components(&self) -> usize {
        self.components.len()
    }

    #[inline]
    pub fn num_rows(&self) -> usize {
        self.row_ids.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num_rows() == 0
    }

    /// Returns the [`RowId`]s in their raw-est form: a tuple of (times, counters) arrays.
    #[inline]
    pub fn row_ids_raw(&self) -> (&ArrowPrimitiveArray<u64>, &ArrowPrimitiveArray<u64>) {
        let [times, counters] = self.row_ids.values() else {
            panic!("RowIds are corrupt -- this should be impossible (sanity checked)");
        };

        #[allow(clippy::unwrap_used)]
        let times = times
            .as_any()
            .downcast_ref::<ArrowPrimitiveArray<u64>>()
            .unwrap(); // sanity checked

        #[allow(clippy::unwrap_used)]
        let counters = counters
            .as_any()
            .downcast_ref::<ArrowPrimitiveArray<u64>>()
            .unwrap(); // sanity checked

        (times, counters)
    }

    #[inline]
    pub fn row_ids(&self) -> impl Iterator<Item = RowId> + '_ {
        let (times, counters) = self.row_ids_raw();
        izip!(times.values().as_slice(), counters.values().as_slice())
            .map(|(&time, &counter)| RowId::from_u128((time as u128) << 64 | (counter as u128)))
    }

    /// Returns the [`RowId`]-range covered by this [`Chunk`].
    ///
    /// `None` if the chunk `is_empty`.
    ///
    /// This is O(1) if the chunk is sorted, O(n) otherwise.
    #[inline]
    pub fn row_id_range(&self) -> Option<(RowId, RowId)> {
        if self.is_empty() {
            return None;
        }

        let (times, counters) = self.row_ids_raw();
        let (times, counters) = (times.values().as_slice(), counters.values().as_slice());

        #[allow(clippy::unwrap_used)] // checked above
        let (index_min, index_max) = if self.is_sorted() {
            (
                (
                    times.first().copied().unwrap(),
                    counters.first().copied().unwrap(),
                ),
                (
                    times.last().copied().unwrap(),
                    counters.last().copied().unwrap(),
                ),
            )
        } else {
            (
                (
                    times.iter().min().copied().unwrap(),
                    counters.iter().min().copied().unwrap(),
                ),
                (
                    times.iter().max().copied().unwrap(),
                    counters.iter().max().copied().unwrap(),
                ),
            )
        };

        let (time_min, counter_min) = index_min;
        let (time_max, counter_max) = index_max;

        Some((
            RowId::from_u128((time_min as u128) << 64 | (counter_min as u128)),
            RowId::from_u128((time_max as u128) << 64 | (counter_max as u128)),
        ))
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        self.timelines.is_empty()
    }

    #[inline]
    pub fn timelines(&self) -> &BTreeMap<Timeline, ChunkTimeline> {
        &self.timelines
    }

    #[inline]
    pub fn component_names(&self) -> impl Iterator<Item = ComponentName> + '_ {
        self.components.keys().copied()
    }

    #[inline]
    pub fn components(&self) -> &BTreeMap<ComponentName, ArrowListArray<i32>> {
        &self.components
    }

    /// Computes the maximum value for each and every timeline present across this entire chunk,
    /// and returns the corresponding [`TimePoint`].
    #[inline]
    pub fn timepoint_max(&self) -> TimePoint {
        self.timelines
            .iter()
            .map(|(timeline, info)| (*timeline, info.time_range.max()))
            .collect()
    }
}

impl std::fmt::Display for Chunk {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chunk = self.to_transport().map_err(|err| {
            re_log::error_once!("couldn't display Chunk: {err}");
            std::fmt::Error
        })?;
        chunk.fmt(f)
    }
}

impl ChunkTimeline {
    #[inline]
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.timeline.name()
    }

    #[inline]
    pub fn time_range(&self) -> ResolvedTimeRange {
        self.time_range
    }

    #[inline]
    pub fn times(&self) -> impl DoubleEndedIterator<Item = TimeInt> + '_ {
        self.times
            .values()
            .as_slice()
            .iter()
            .copied()
            .map(TimeInt::new_temporal)
    }

    #[inline]
    pub fn times_raw(&self) -> &[i64] {
        self.times.values().as_slice()
    }

    #[inline]
    pub fn num_rows(&self) -> usize {
        self.times.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num_rows() == 0
    }

    /// Computes the time range covered by each individual component column.
    ///
    /// This is different from the time range covered by the [`ChunkTimeline`] as a whole
    /// because component columns are potentially sparse.
    ///
    /// This is crucial for indexing and queries to work properly.
    //
    // TODO(cmc): This needs to be stored in chunk metadata and transported across IPC.
    pub fn time_range_per_component(
        &self,
        components: &BTreeMap<ComponentName, ArrowListArray<i32>>,
    ) -> BTreeMap<ComponentName, ResolvedTimeRange> {
        let times = self.times_raw();
        components
            .iter()
            .filter_map(|(&component_name, list_array)| {
                if let Some(validity) = list_array.validity() {
                    // _Potentially_ sparse

                    if validity.is_empty() {
                        return None;
                    }

                    let is_dense = validity.unset_bits() == 0;
                    if is_dense {
                        return Some((component_name, self.time_range));
                    }

                    let mut time_min = TimeInt::MAX;
                    for (i, time) in times.iter().copied().enumerate() {
                        if validity.get(i).unwrap_or(false) {
                            time_min = TimeInt::new_temporal(time);
                            break;
                        }
                    }

                    let mut time_max = TimeInt::MIN;
                    for (i, time) in times.iter().copied().enumerate().rev() {
                        if validity.get(i).unwrap_or(false) {
                            time_max = TimeInt::new_temporal(time);
                            break;
                        }
                    }

                    Some((component_name, ResolvedTimeRange::new(time_min, time_max)))
                } else {
                    // Dense

                    Some((component_name, self.time_range))
                }
            })
            .collect()
    }
}

impl re_types_core::SizeBytes for Chunk {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            id,
            entity_path,
            heap_size_bytes,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        let mut size_bytes = heap_size_bytes.load(Ordering::Relaxed);

        if size_bytes == 0 {
            size_bytes = id.heap_size_bytes()
                + entity_path.heap_size_bytes()
                + is_sorted.heap_size_bytes()
                + row_ids.heap_size_bytes()
                + timelines.heap_size_bytes()
                + components.heap_size_bytes();
            heap_size_bytes.store(size_bytes, Ordering::Relaxed);
        }

        size_bytes
    }
}

impl re_types_core::SizeBytes for ChunkTimeline {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range,
        } = self;

        timeline.heap_size_bytes()
            + times.heap_size_bytes() // cheap
            + is_sorted.heap_size_bytes()
            + time_range.heap_size_bytes()
    }
}

// TODO(cmc): methods to merge chunks (compaction).

// --- Sanity checks ---

impl Chunk {
    /// Returns an error if the Chunk's invariants are not upheld.
    ///
    /// Costly checks are only run in debug builds.
    pub fn sanity_check(&self) -> ChunkResult<()> {
        re_tracing::profile_function!();

        let Self {
            id: _,
            entity_path: _,
            heap_size_bytes,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        #[allow(clippy::collapsible_if)] // readability
        if cfg!(debug_assertions) {
            let measured = self.heap_size_bytes();
            let advertised = heap_size_bytes.load(Ordering::Relaxed);
            if advertised != measured {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "Chunk advertises a heap size of {} but we measure {} instead",
                        re_format::format_bytes(advertised as _),
                        re_format::format_bytes(measured as _),
                    ),
                });
            }
        }

        // Row IDs
        {
            if *row_ids.data_type().to_logical_type() != RowId::arrow_datatype() {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "RowId data has the wrong datatype: expected {:?} but got {:?} instead",
                        RowId::arrow_datatype(),
                        *row_ids.data_type(),
                    ),
                });
            }

            #[allow(clippy::collapsible_if)] // readability
            if cfg!(debug_assertions) {
                if *is_sorted != self.is_sorted_uncached() {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "Chunk is marked as {}sorted but isn't: {row_ids:?}",
                            if *is_sorted { "" } else { "un" },
                        ),
                    });
                }
            }
        }

        // Timelines
        for (timeline, time_chunk) in timelines {
            if time_chunk.times.len() != row_ids.len() {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "All timelines in a chunk must have the same number of timestamps, matching the number of row IDs.\
                         Found {} row IDs but {} timestamps for timeline {:?}",
                        row_ids.len(), time_chunk.times.len(), timeline.name(),
                    ),
                });
            }

            time_chunk.sanity_check()?;
        }

        // Components
        for (component_name, list_array) in components {
            if !matches!(list_array.data_type(), arrow2::datatypes::DataType::List(_)) {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "The outer array in a chunked component batch must be a sparse list, got {:?}",
                        list_array.data_type(),
                    ),
                });
            }
            if let arrow2::datatypes::DataType::List(field) = list_array.data_type() {
                if !field.is_nullable {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "The outer array in chunked component batch must be a sparse list, got {:?}",
                            list_array.data_type(),
                        ),
                    });
                }
            }
            if list_array.len() != row_ids.len() {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "All component batches in a chunk must have the same number of rows, matching the number of row IDs.\
                         Found {} row IDs but {} rows for component batch {component_name}",
                        row_ids.len(), list_array.len(),
                    ),
                });
            }

            let validity_is_empty = list_array
                .validity()
                .map_or(false, |validity| validity.is_empty());
            if !self.is_empty() && validity_is_empty {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "All component batches in a chunk must contain at least one non-null entry.\
                         Found a completely empty column for {component_name}",
                    ),
                });
            }
        }

        Ok(())
    }
}

impl ChunkTimeline {
    /// Returns an error if the Chunk's invariants are not upheld.
    ///
    /// Costly checks are only run in debug builds.
    pub fn sanity_check(&self) -> ChunkResult<()> {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range,
        } = self;

        if *times.data_type() != timeline.datatype() {
            return Err(ChunkError::Malformed {
                reason: format!(
                    "Time data for timeline {} has the wrong datatype: expected {:?} but got {:?} instead",
                    timeline.name(),
                    timeline.datatype(),
                    *times.data_type(),
                ),
            });
        }

        let times = times.values().as_slice();

        #[allow(clippy::collapsible_if)] // readability
        if cfg!(debug_assertions) {
            if *is_sorted != times.windows(2).all(|times| times[0] <= times[1]) {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "Chunk timeline is marked as {}sorted but isn't: {times:?}",
                        if *is_sorted { "" } else { "un" },
                    ),
                });
            }
        }

        #[allow(clippy::collapsible_if)] // readability
        if cfg!(debug_assertions) {
            let is_tight_lower_bound = times.iter().any(|&time| time == time_range.min().as_i64());
            let is_tight_upper_bound = times.iter().any(|&time| time == time_range.max().as_i64());
            let is_tight_bound = is_tight_lower_bound && is_tight_upper_bound;

            if !self.is_empty() && !is_tight_bound {
                return Err(ChunkError::Malformed {
                    reason: "Chunk timeline's cached time range isn't a tight bound.".to_owned(),
                });
            }

            for &time in times {
                if time < time_range.min().as_i64() || time > time_range.max().as_i64() {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "Chunk timeline's cached time range is wrong.\
                             Found a time value of {time} while its time range is {time_range:?}",
                        ),
                    });
                }

                if time == TimeInt::STATIC.as_i64() {
                    return Err(ChunkError::Malformed {
                        reason: "A chunk's timeline should never contain a static time value."
                            .to_owned(),
                    });
                }
            }
        }

        Ok(())
    }
}
