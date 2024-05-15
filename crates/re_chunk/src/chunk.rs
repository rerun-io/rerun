use std::collections::BTreeMap;

use arrow2::array::Array as ArrowArray;

use re_log_types::{EntityPath, ResolvedTimeRange, RowId, TimeInt, TimePoint, Timeline};
use re_types_core::{ComponentName, SerializationError};

// ---

/// Errors that can occur when creating/manipulating a [`ChunkBatcher`].
#[derive(thiserror::Error, Debug)]
pub enum ChunkError {
    /// The chunk doesn't obey the chunking rules.
    #[error("Detected malformed Chunk: {reason}")]
    Malformed { reason: String },

    #[error(transparent)]
    Serialization(#[from] SerializationError),

    #[error("Chunks cannot be empty")]
    Empty,
}

pub type ChunkResult<T> = Result<T, ChunkError>;

// TODO(cmc): sizebytes impl + sizebytes caching + sizebytes in transport metadata

// ---

/// Dense arrow-based storage of N rows of multi-component temporal data for a specific entity.
///
/// This is our core datastructure for logging, storing, querying and transporting data around.
///
/// The chunk as a whole is always ascendingly sorted by [`RowId`] before it gets manipulated in any way.
/// Its time columns might or might not be ascendingly sorted, depending on how the data was logged.
///
/// This is the in-memory representation of a chunk, optimized for efficient manipulation of the
/// data within. For transport, see [`TransportChunk`].
#[derive(Debug, Clone)]
pub struct Chunk {
    pub(crate) entity_path: EntityPath,

    /// Is the chunk as a whole sorted?
    pub(crate) is_sorted: bool,

    /// The respective [`RowId`]s for each row of data.
    ///
    /// This is always ascendingly sorted, and defines the order of the chunk as a whole.
    pub(crate) row_ids: Vec<RowId>,

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
    /// Sparse so that we can e.g. log only `Position` one time, but not a `Color`.
    pub(crate) components: BTreeMap<ComponentName, Box<dyn ArrowArray>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkTimeline {
    /// Every single timestamp for this timeline.
    ///
    /// * This might or might not be sorted, depending on how the data was logged.
    /// * This is guaranteed to always be dense, because chunks are split anytime a timeline is
    ///   added or removed.
    /// * This can never contain `TimeInt::STATIC` , since static data doesn't even have timelines.
    //
    // TODO(cmc): maybe this would be better as a raw i64 so getting time columns in and out and
    // chunks is just a blind memcpy… it's probably not worth the hassle.
    pub(crate) times: Vec<TimeInt>,

    /// Is [`Self::times`] sorted?
    pub(crate) is_sorted: bool,

    /// The time range covered by [`Self::times`].
    ///
    /// Not necessarily contiguous! Just the min and max value in [`Self::times`].
    pub(crate) time_range: ResolvedTimeRange,
}

#[cfg(test)] // do not ever use this outside internal testing, it's extremely slow and hackish
impl PartialEq for Chunk {
    #[inline]
    fn eq(&self, rhs: &Self) -> bool {
        let Self {
            entity_path,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        use itertools::Itertools as _;

        *entity_path == rhs.entity_path
            && *is_sorted == rhs.is_sorted
            && *row_ids == rhs.row_ids
            && *timelines == rhs.timelines
            && components.keys().collect_vec() == rhs.components.keys().collect_vec()
            && components.iter().all(|(component_name, list_array)| {
                let Some(rhs_list_array) = rhs
                    .components
                    .get(component_name)
                    .map(|list_array| &**list_array)
                else {
                    return false;
                };

                // `arrow2::compute::comparison` has very limited support for the different arrow
                // types, so we just do our best here.
                if arrow2::compute::comparison::can_eq(list_array.data_type()) {
                    arrow2::compute::comparison::eq(&**list_array, rhs_list_array)
                        .values_iter()
                        .all(|v| v)
                } else {
                    list_array.data_type() == rhs_list_array.data_type()
                        && list_array.len() == rhs_list_array.len()
                }
            })
    }
}

#[cfg(test)] // do not ever use this outside internal testing, it's extremely slow and hackish
impl Eq for Chunk {}

// ---

impl Chunk {
    /// Creates a new [`Chunk`].
    ///
    /// This will fail if the passed in data is malformed in any way -- see [`Self::sanity_check`]
    /// for details.
    ///
    /// Iff you know for sure whether the data is already appropriately sorted, specify `is_sorted`.
    pub fn new(
        entity_path: EntityPath,
        is_sorted: Option<bool>,
        row_ids: Vec<RowId>,
        timelines: BTreeMap<Timeline, Vec<TimeInt>>,
        components: BTreeMap<ComponentName, Box<dyn ArrowArray>>,
    ) -> ChunkResult<Self> {
        if row_ids.is_empty() {
            return Err(ChunkError::Empty);
        }

        let mut chunk = Self {
            entity_path,
            is_sorted: false,
            row_ids,
            timelines: timelines
                .into_iter()
                .filter_map(|(timeline, times)| {
                    if times.is_empty() {
                        return None;
                    }

                    // NOTE: Do the iteration multiple times in a cache-friendly way rather than the opposite.
                    // NOTE: The 'or' in 'unwrap_or' is never hit, but better safe than sorry.
                    let min_time = times.iter().min().copied().unwrap_or(TimeInt::MIN);
                    let max_time = times.iter().max().copied().unwrap_or(TimeInt::MAX);
                    let is_sorted = times.windows(2).all(|times| times[0] <= times[1]);

                    Some((
                        timeline,
                        ChunkTimeline {
                            times,
                            is_sorted,
                            time_range: ResolvedTimeRange::new(min_time, max_time),
                        },
                    ))
                })
                .collect(),
            components,
        };

        chunk.is_sorted = is_sorted.unwrap_or_else(|| chunk.is_sorted_uncached());

        chunk.sanity_check()?;

        Ok(chunk)
    }

    #[inline]
    pub fn new_static(
        entity_path: EntityPath,
        is_sorted: Option<bool>,
        row_ids: Vec<RowId>,
        components: BTreeMap<ComponentName, Box<dyn ArrowArray>>,
    ) -> ChunkResult<Self> {
        Self::new(
            entity_path,
            is_sorted,
            row_ids,
            Default::default(),
            components,
        )
    }

    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }

    /// How many columns total? Includes control, time, and component columns.
    #[inline]
    pub fn num_columns(&self) -> usize {
        let Self {
            entity_path: _, // not an actual column
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

    /// Returns the first [`RowId`] in this [`Chunk`].
    ///
    /// That is also the smallest [`RowId`] if the chunk is sorted, otherwise it can be anything.
    /// The caller is responsible for sorting the chunk.
    #[inline]
    pub fn first_row_id(&self) -> RowId {
        #[allow(clippy::unwrap_used)] // cannot create empty chunks
        self.row_ids.first().copied().unwrap()
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chunk = self.to_transport().map_err(|err| {
            re_log::error_once!("couldn't display Chunk: {err}");
            std::fmt::Error
        })?;

        f.write_fmt(format_args!(
            "Chunk({:?} ({})):\n",
            self.entity_path,
            self.first_row_id()
        ))?;
        re_format_arrow::format_table(
            chunk.data.columns(),
            chunk.schema.fields.iter().map(|field| field.name.as_str()),
        )
        .fmt(f)
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
            entity_path: _,
            is_sorted,
            row_ids,
            timelines,
            components,
        } = self;

        if row_ids.is_empty() || components.is_empty() {
            return Err(ChunkError::Empty);
        }

        // Row IDs
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

        // Timelines
        for (timeline, info) in timelines {
            let ChunkTimeline {
                times,
                is_sorted,
                time_range,
            } = info;
            if info.times.len() != row_ids.len() {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "All timelines in a chunk must have the same number of timestamps, matching the number of row IDs.\
                         Found {} row IDs but {} timestamps for timeline {:?}",
                        row_ids.len(), times.len(), timeline.name(),
                    ),
                });
            }

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
                for &time in times {
                    if time < time_range.min() || time > time_range.max() {
                        return Err(ChunkError::Malformed {
                            reason: format!(
                                "Chunk timeline's cached time range is wrong.\
                                 Found a value of {} timeline {:?} while its time range is {time_range:?}",
                                time.as_i64(),
                                timeline.name(),
                            ),
                        });
                    }

                    if time.is_static() {
                        return Err(ChunkError::Malformed {
                            reason: format!(
                                "A chunk's timeline should never contain a static time value.\
                                 Found a static value in timeline {:?}",
                                timeline.name(),
                            ),
                        });
                    }
                }
            }
        }

        // Components
        for (component_name, list_array) in components {
            if !matches!(list_array.data_type(), arrow2::datatypes::DataType::List(_)) {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "The outer array in chunked component batch must be a sparse list, got {:?}",
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
        }

        Ok(())
    }
}
