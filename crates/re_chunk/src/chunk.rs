use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

use arrow2::array::{
    Array as ArrowArray, ListArray as ArrowListArray, PrimitiveArray as ArrowPrimitiveArray,
    StructArray as ArrowStructArray,
};

use itertools::{izip, Itertools};
use re_log_types::{EntityPath, ResolvedTimeRange, RowId, TimeInt, TimePoint, Timeline};
use re_types_core::{ComponentName, Loggable, LoggableBatch, SerializationError, SizeBytes};

// TODO: we're going to need a chunk iterator for sure, where the cost of downcasting etc is only
// paid when creating the iterator itself.

// TODO: would be nice to offer a helper to merge N chunks into a pure arrow chunk, that doesnt
// need to respect the usual split conditions (e.g. to print a giant dataframe of the entire
// store).

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

// TODO: the store ID should be in the metadata here so we can remove the layer on top

/// Unique identifier for a [`Chunk`], using a [`re_tuid::Tuid`].
// TODO: should we declare an actual type like with rowid?
pub type ChunkId = re_tuid::Tuid;

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
    // TODO
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
            timelines: timelines
                .into_iter()
                // TODO: what's with the filter?
                .filter(|(_, time_chunk)| !time_chunk.times.is_empty())
                .collect(),
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

    /// Simple helper for [`Self::new`] for static data.
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
}

impl ChunkTimeline {
    /// Creates a new [`ChunkTimeline`].
    ///
    /// Returns `None` if `times` is empty.
    ///
    /// Iff you know for sure whether the data is already appropriately sorted or not, specify `is_sorted`.
    /// When left unspecified (`None`), it will be computed in O(n) time.
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

    // TODO: doc
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
    /// This is O(1) if the chunk is sorted, O(n) otherwise.
    #[inline]
    pub fn row_id_range(&self) -> (RowId, RowId) {
        let (times, counters) = self.row_ids_raw();
        let (times, counters) = (times.values().as_slice(), counters.values().as_slice());

        #[allow(clippy::unwrap_used)] // cannot create empty chunks
        if self.is_sorted() {
            (
                {
                    let time = times.first().copied().unwrap();
                    let counter = counters.first().copied().unwrap();
                    RowId::from_u128((time as u128) << 64 | (counter as u128))
                },
                {
                    let time = times.last().copied().unwrap();
                    let counter = counters.last().copied().unwrap();
                    RowId::from_u128((time as u128) << 64 | (counter as u128))
                },
            )
        } else {
            (
                {
                    let time = times.iter().min().copied().unwrap();
                    let counter = counters.iter().min().copied().unwrap();
                    RowId::from_u128((time as u128) << 64 | (counter as u128))
                },
                {
                    let time = times.iter().max().copied().unwrap();
                    let counter = counters.iter().max().copied().unwrap();
                    RowId::from_u128((time as u128) << 64 | (counter as u128))
                },
            )
        }
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
    pub fn time_range(&self) -> ResolvedTimeRange {
        self.time_range
    }

    #[inline]
    pub fn times(&self) -> &[i64] {
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
