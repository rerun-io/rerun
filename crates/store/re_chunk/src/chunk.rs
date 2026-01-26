use std::sync::atomic::{AtomicU64, Ordering};

use ahash::HashMap;
use anyhow::Context as _;
use arrow::array::{
    Array as ArrowArray, ArrayRef as ArrowArrayRef, FixedSizeBinaryArray,
    ListArray as ArrowListArray,
};
use arrow::buffer::{NullBuffer as ArrowNullBuffer, ScalarBuffer as ArrowScalarBuffer};
use itertools::{Either, Itertools as _, izip};
use nohash_hasher::IntMap;
use re_arrow_util::{ArrowArrayDowncastRef as _, widen_binary_arrays};
use re_byte_size::SizeBytes as _;
use re_log_types::{
    AbsoluteTimeRange, EntityPath, NonMinI64, TimeInt, TimeType, Timeline, TimelineName,
};
use re_types_core::{
    ComponentDescriptor, ComponentIdentifier, ComponentType, DeserializationError, Loggable as _,
    SerializationError, SerializedComponentColumn,
};

use crate::{ChunkId, RowId};

// ---

/// Errors that can occur when creating/manipulating a [`Chunk`]s, directly or indirectly through
/// the use of a [`crate::ChunkBatcher`].
#[derive(thiserror::Error, Debug)]
pub enum ChunkError {
    #[error("Detected malformed Chunk: {reason}")]
    Malformed { reason: String },

    #[error("Arrow: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("{kind} index out of bounds: {index} (len={len})")]
    IndexOutOfBounds {
        kind: String,
        len: usize,
        index: usize,
    },

    #[error("Serialization: {0}")]
    Serialization(#[from] SerializationError),

    #[error("Deserialization: {0}")]
    Deserialization(#[from] DeserializationError),

    #[error(transparent)]
    UnsupportedTimeType(#[from] re_sorbet::UnsupportedTimeType),

    #[error(transparent)]
    WrongDatatypeError(#[from] re_arrow_util::WrongDatatypeError),

    #[error(transparent)]
    MismatchedChunkSchemaError(#[from] re_sorbet::MismatchedChunkSchemaError),

    #[error(transparent)]
    InvalidSorbetSchema(#[from] re_sorbet::SorbetError),
}

const _: () = assert!(
    std::mem::size_of::<ChunkError>() <= 72,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

pub type ChunkResult<T> = Result<T, ChunkError>;

// ---

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChunkComponents(pub IntMap<ComponentIdentifier, SerializedComponentColumn>);

impl ChunkComponents {
    /// Returns all list arrays for the given component type.
    ///
    /// I.e semantically equivalent to `get("MyComponent:*.*")`
    #[inline]
    pub fn get_by_component_type(
        &self,
        component_type: ComponentType,
    ) -> impl Iterator<Item = &ArrowListArray> {
        self.0.values().filter_map(move |column| {
            (column.descriptor.component_type == Some(component_type)).then_some(&column.list_array)
        })
    }

    /// Approximate equal, that ignores small numeric differences.
    ///
    /// Returns `Ok` if similar.
    /// If there is a difference, a description of that difference is returned in `Err`.
    /// We use [`anyhow`] to provide context.
    ///
    /// Useful for tests.
    pub fn ensure_similar(left: &Self, right: &Self) -> anyhow::Result<()> {
        anyhow::ensure!(left.len() == right.len());
        for (component, left_column) in left.iter() {
            let Some(right_column) = right.get(*component) else {
                anyhow::bail!("rhs is missing {component:?}");
            };
            anyhow::ensure!(left_column.descriptor == right_column.descriptor);

            let left_array = widen_binary_arrays(&left_column.list_array);
            let right_array = widen_binary_arrays(&right_column.list_array);
            re_arrow_util::ensure_similar(&left_array.to_data(), &right_array.to_data())
                .with_context(|| format!("Component {component:?}"))?;
        }
        Ok(())
    }

    /// Whether any of the components in this chunk has the given name.
    #[inline]
    pub fn contains_component(&self, component: ComponentIdentifier) -> bool {
        self.0.contains_key(&component)
    }

    /// Lists all the component descriptors in this chunk.
    #[inline]
    pub fn component_descriptors(&self) -> impl Iterator<Item = &ComponentDescriptor> + '_ {
        self.0.values().map(|column| &column.descriptor)
    }

    /// Lists all the component list arrays in this chunk.
    #[inline]
    pub fn list_arrays(&self) -> impl Iterator<Item = &ArrowListArray> + '_ {
        self.0.values().map(|column| &column.list_array)
    }

    /// Lists all the component list arrays in this chunk.
    #[inline]
    pub fn list_arrays_mut(&mut self) -> impl Iterator<Item = &mut ArrowListArray> + '_ {
        self.0.values_mut().map(|column| &mut column.list_array)
    }

    /// Returns the array for a given component if any.
    #[inline]
    pub fn get_array(&self, component: ComponentIdentifier) -> Option<&ArrowListArray> {
        self.0.get(&component).map(|column| &column.list_array)
    }

    /// Returns the descriptor for a given component if any.
    #[inline]
    pub fn get_descriptor(&self, component: ComponentIdentifier) -> Option<&ComponentDescriptor> {
        self.0.get(&component).map(|column| &column.descriptor)
    }

    /// Returns the descriptor and array for a given component if any.
    #[inline]
    pub fn get(&self, component: ComponentIdentifier) -> Option<&SerializedComponentColumn> {
        self.0.get(&component)
    }

    /// Unconditionally inserts a [`SerializedComponentColumn`].
    ///
    /// Removes and replaces the column if it already exists.
    #[inline]
    pub fn insert(
        &mut self,
        column: SerializedComponentColumn,
    ) -> Option<SerializedComponentColumn> {
        self.0.insert(column.descriptor.component, column)
    }
}

impl std::ops::Deref for ChunkComponents {
    type Target = IntMap<ComponentIdentifier, SerializedComponentColumn>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for ChunkComponents {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// TODO(andreas): Remove this variant, we should let users construct `SerializedComponentColumn` directly to sharpen semantics.
impl FromIterator<(ComponentDescriptor, ArrowListArray)> for ChunkComponents {
    #[inline]
    fn from_iter<T: IntoIterator<Item = (ComponentDescriptor, ArrowListArray)>>(iter: T) -> Self {
        let mut this = Self::default();
        {
            for (component_desc, list_array) in iter {
                this.insert(SerializedComponentColumn::new(list_array, component_desc));
            }
        }
        this
    }
}

impl FromIterator<SerializedComponentColumn> for ChunkComponents {
    #[inline]
    fn from_iter<T: IntoIterator<Item = SerializedComponentColumn>>(iter: T) -> Self {
        let mut this = Self::default();
        {
            for serialized in iter {
                this.insert(serialized);
            }
        }
        this
    }
}

/// Dense arrow-based storage of N rows of multi-component multi-temporal data for a specific entity.
///
/// This is our core datastructure for logging, storing, querying and transporting data around.
///
/// The chunk as a whole is always ascendingly sorted by [`RowId`] before it gets manipulated in any way.
/// Its time columns might or might not be ascendingly sorted, depending on how the data was logged.
///
/// This is the in-memory representation of a chunk, optimized for efficient manipulation of the
/// data within. For transport, see [`re_sorbet::ChunkBatch`] instead.
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
    pub(crate) row_ids: FixedSizeBinaryArray,

    /// The time columns.
    ///
    /// Each column must be the same length as `row_ids`.
    ///
    /// Empty if this is a static chunk.
    pub(crate) timelines: IntMap<TimelineName, TimeColumn>,

    /// A sparse `ListArray` & a [`ComponentDescriptor`] for each component.
    ///
    /// Each `ListArray` must be the same length as `row_ids`.
    ///
    /// Sparse so that we can e.g. log a `Position` at one timestamp but not a `Color`.
    pub(crate) components: ChunkComponents,
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
    /// Returns a version of us with a new [`ChunkId`].
    ///
    /// Reminder:
    /// * The returned [`Chunk`] will re-use the exact same [`RowId`]s as `self`.
    /// * Duplicated [`RowId`]s in the `ChunkStore` is undefined behavior.
    #[must_use]
    #[inline]
    pub fn with_id(mut self, id: ChunkId) -> Self {
        self.id = id;
        self
    }

    /// Returns `Ok` if two [`Chunk`]s are _similar_, although not byte-for-byte equal.
    ///
    /// In particular, this ignores chunks and row IDs, as well as `log_time` timestamps.
    /// It also forgives small numeric inaccuracies in floating point buffers.
    ///
    /// If there is a difference, a description of that difference is returned in `Err`.
    /// We use [`anyhow`] to provide context.
    ///
    /// Useful for tests.
    pub fn ensure_similar(lhs: &Self, rhs: &Self) -> anyhow::Result<()> {
        anyhow::ensure!(lhs.num_rows() == rhs.num_rows());
        anyhow::ensure!(lhs.num_columns() == rhs.num_columns());

        let Self {
            id: _,
            entity_path,
            heap_size_bytes: _,
            is_sorted: _,
            row_ids: _,
            timelines,
            components,
        } = lhs;

        anyhow::ensure!(*entity_path == rhs.entity_path);

        anyhow::ensure!(timelines.keys().collect_vec() == rhs.timelines.keys().collect_vec());

        for (timeline, left_time_col) in timelines {
            let right_time_col = rhs
                .timelines
                .get(timeline)
                .ok_or_else(|| anyhow::format_err!("right is missing timeline {timeline:?}"))?;
            if timeline == &TimelineName::log_time() {
                continue; // We expect this to differ
            }
            if timeline == "sim_time" {
                continue; // Small numeric differences
            }
            anyhow::ensure!(
                left_time_col == right_time_col,
                "Timeline differs: {timeline:?}"
            );
        }

        // Handle edge case: recording time on segment properties should ignore start time.
        if entity_path == &EntityPath::properties() {
            // We're going to filter out some components on both lhs and rhs.
            // Therefore, it's important that we first check that the number of components is the same.
            anyhow::ensure!(components.len() == rhs.components.len());

            // Copied from `rerun.archetypes.RecordingInfo`.
            let recording_time_component: ComponentIdentifier = "RecordingInfo:start_time".into();

            // Filter out the recording time component from both lhs and rhs.
            let lhs_components = components
                .iter()
                .filter(|&(component, _list_array)| component != &recording_time_component)
                .map(|(component, list_array)| (*component, list_array.clone()))
                .collect::<IntMap<_, _>>();
            let rhs_components = rhs
                .components
                .iter()
                .filter(|&(component, _list_array)| component != &recording_time_component)
                .map(|(component, list_array)| (*component, list_array.clone()))
                .collect::<IntMap<_, _>>();

            anyhow::ensure!(lhs_components == rhs_components);
            Ok(())
        } else {
            ChunkComponents::ensure_similar(components, &rhs.components)
        }
    }

    // Only used for tests atm
    pub fn are_equal(&self, other: &Self) -> bool {
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
            && row_ids == &other.row_ids
            && *timelines == other.timelines
            && components.0 == other.components.0
    }

    /// Clones the chunk and renames a component.
    ///
    /// Note: archetype information and component type information is lost.
    pub fn with_renamed_component(
        &self,
        selector: ComponentIdentifier,
        target: ComponentIdentifier,
    ) -> Self {
        let mut new_chunk = self.clone();
        if let Some(old_entry) = new_chunk.components.remove(&selector) {
            new_chunk.components.insert(SerializedComponentColumn {
                descriptor: ComponentDescriptor {
                    component: target,
                    archetype: None,
                    component_type: None,
                },
                list_array: old_entry.list_array,
            });
        }

        new_chunk
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

        Self {
            id,
            row_ids: RowId::arrow_from_slice(&row_ids),
            ..self.clone()
        }
    }

    /// Clones the chunk into a new chunk without any time data.
    #[inline]
    pub fn into_static(mut self) -> Self {
        self.timelines.clear();
        self
    }

    /// Clones the chunk into a new chunk where all [`RowId`]s are [`RowId::ZERO`].
    pub fn zeroed(self) -> Self {
        let row_ids = vec![RowId::ZERO; self.row_ids.len()];

        let row_ids = RowId::arrow_from_slice(&row_ids);

        Self { row_ids, ..self }
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
    ) -> IntMap<TimelineName, IntMap<ComponentIdentifier, AbsoluteTimeRange>> {
        re_tracing::profile_function!();

        self.timelines
            .iter()
            .map(|(timeline_name, time_column)| {
                (
                    *timeline_name,
                    time_column.time_range_per_component(&self.components),
                )
            })
            .collect()
    }

    #[inline]
    pub fn component_descriptors(&self) -> impl Iterator<Item = &ComponentDescriptor> + '_ {
        self.components.component_descriptors()
    }

    /// The cumulative number of events in this chunk.
    ///
    /// I.e. how many _component batches_ ("cells") were logged in total?
    //
    // TODO(cmc): This needs to be stored in chunk metadata and transported across IPC.
    #[inline]
    pub fn num_events_cumulative(&self) -> u64 {
        // Reminder: component columns are sparse, we must take a look at the validity bitmaps.
        self.components
            .list_arrays()
            .map(|list_array| {
                list_array.nulls().map_or_else(
                    || list_array.len() as u64,
                    |validity| validity.len() as u64 - validity.null_count() as u64,
                )
            })
            .sum()
    }

    /// The cumulative number of events in this chunk for each _unique_ timestamp.
    ///
    /// I.e. how many _component batches_ ("cells") were logged in total at each timestamp?
    ///
    /// Keep in mind that a timestamp can appear multiple times in a [`Chunk`].
    /// This method will do a sum accumulation to account for these cases (i.e. every timestamp in
    /// the returned vector is guaranteed to be unique).
    pub fn num_events_cumulative_per_unique_time(
        &self,
        timeline: &TimelineName,
    ) -> Vec<(TimeInt, u64)> {
        if self.is_static() {
            return vec![(TimeInt::STATIC, self.num_events_cumulative())];
        }

        let Some(time_column) = self.timelines().get(timeline) else {
            return Vec::new();
        };

        let time_range = time_column.time_range();
        if time_range.min() == time_range.max() {
            return vec![(time_range.min(), self.num_events_cumulative())];
        }

        let counts = if time_column.is_sorted() {
            self.num_events_cumulative_per_unique_time_sorted(time_column)
        } else {
            self.num_events_cumulative_per_unique_time_unsorted(time_column)
        };

        debug_assert!(
            counts
                .iter()
                .tuple_windows::<(_, _)>()
                .all(|((time1, _), (time2, _))| time1 < time2)
        );

        counts
    }

    fn num_events_cumulative_per_unique_time_sorted(
        &self,
        time_column: &TimeColumn,
    ) -> Vec<(TimeInt, u64)> {
        debug_assert!(time_column.is_sorted());

        // NOTE: This is used on some very hot paths (time panel rendering).
        // Performance trumps readability. Optimized empirically.

        // Raw, potentially duplicated counts (because timestamps aren't necessarily unique).
        let mut counts_raw = vec![0u64; self.num_rows()];
        {
            self.components.list_arrays().for_each(|list_array| {
                if let Some(validity) = list_array.nulls() {
                    validity
                        .iter()
                        .enumerate()
                        .for_each(|(i, is_valid)| counts_raw[i] += is_valid as u64);
                } else {
                    for count in &mut counts_raw {
                        *count += 1;
                    }
                }
            });
        }

        let mut counts = Vec::with_capacity(counts_raw.len());

        let Some(mut cur_time) = time_column.times().next() else {
            return Vec::new();
        };
        let mut cur_count = 0;
        izip!(time_column.times(), counts_raw).for_each(|(time, count)| {
            if time == cur_time {
                cur_count += count;
            } else {
                counts.push((cur_time, cur_count));
                cur_count = count;
                cur_time = time;
            }
        });

        if counts.last().map(|(time, _)| *time) != Some(cur_time) {
            counts.push((cur_time, cur_count));
        }

        counts
    }

    fn num_events_cumulative_per_unique_time_unsorted(
        &self,
        time_column: &TimeColumn,
    ) -> Vec<(TimeInt, u64)> {
        debug_assert!(!time_column.is_sorted());

        // NOTE: This is used on some very hot paths (time panel rendering).

        let result_unordered =
            self.components
                .list_arrays()
                .fold(HashMap::default(), |acc, list_array| {
                    if let Some(validity) = list_array.nulls() {
                        time_column.times().zip(validity.iter()).fold(
                            acc,
                            |mut acc, (time, is_valid)| {
                                *acc.entry(time).or_default() += is_valid as u64;
                                acc
                            },
                        )
                    } else {
                        time_column.times().fold(acc, |mut acc, time| {
                            *acc.entry(time).or_default() += 1;
                            acc
                        })
                    }
                });

        let mut result = result_unordered.into_iter().collect_vec();
        result.sort_by_key(|val| val.0);
        result
    }

    /// The number of events in this chunk for the specified component.
    ///
    /// I.e. how many _component batches_ ("cells") were logged in total for this component?
    //
    // TODO(cmc): This needs to be stored in chunk metadata and transported across IPC.
    #[inline]
    pub fn num_events_for_component(&self, component: ComponentIdentifier) -> Option<u64> {
        // Reminder: component columns are sparse, we must check validity bitmap.
        self.components.get_array(component).map(|list_array| {
            list_array.nulls().map_or_else(
                || list_array.len() as u64,
                |validity| validity.len() as u64 - validity.null_count() as u64,
            )
        })
    }

    /// Computes the `RowId` range covered by each individual component column on each timeline.
    ///
    /// This is different from the `RowId` range covered by the [`Chunk`] as a whole because component
    /// columns are potentially sparse.
    ///
    /// This is crucial for indexing and queries to work properly.
    //
    // TODO(cmc): This needs to be stored in chunk metadata and transported across IPC.
    pub fn row_id_range_per_component(&self) -> IntMap<ComponentIdentifier, (RowId, RowId)> {
        re_tracing::profile_function!();

        let row_ids = self.row_ids().collect_vec();

        if self.is_sorted() {
            self.components
                .iter()
                .filter_map(|(component, column)| {
                    let mut row_id_min = None;
                    let mut row_id_max = None;

                    for (i, &row_id) in row_ids.iter().enumerate() {
                        if column.list_array.is_valid(i) {
                            row_id_min = Some(row_id);
                        }
                    }
                    for (i, &row_id) in row_ids.iter().enumerate().rev() {
                        if column.list_array.is_valid(i) {
                            row_id_max = Some(row_id);
                        }
                    }

                    Some((*component, (row_id_min?, row_id_max?)))
                })
                .collect()
        } else {
            self.components
                .iter()
                .filter_map(|(component, column)| {
                    let mut row_id_min = Some(RowId::MAX);
                    let mut row_id_max = Some(RowId::ZERO);

                    for (i, &row_id) in row_ids.iter().enumerate() {
                        if column.list_array.is_valid(i) && Some(row_id) > row_id_min {
                            row_id_min = Some(row_id);
                        }
                    }
                    for (i, &row_id) in row_ids.iter().enumerate().rev() {
                        if column.list_array.is_valid(i) && Some(row_id) < row_id_max {
                            row_id_max = Some(row_id);
                        }
                    }

                    Some((*component, (row_id_min?, row_id_max?)))
                })
                .collect()
        }
    }
}

// ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeColumn {
    pub(crate) timeline: Timeline,

    /// Every single timestamp for this timeline.
    ///
    /// * This might or might not be sorted, depending on how the data was logged.
    /// * This is guaranteed to always be dense, because chunks are split anytime a timeline is
    ///   added or removed.
    /// * This cannot ever contain `TimeInt::STATIC`, since static data doesn't even have timelines.
    ///
    /// When this buffer is converted to an arrow array, it's datatype will depend
    /// on the timeline type, so it will either become a
    /// [`arrow::array::Int64Array`] or a [`arrow::array::TimestampNanosecondArray`].
    pub(crate) times: ArrowScalarBuffer<i64>,

    /// Is [`Self::times`] sorted?
    ///
    /// This is completely independent of [`Chunk::is_sorted`]: a timeline doesn't necessarily
    /// follow the global [`RowId`]-based order, although it does in most cases (happy path).
    pub(crate) is_sorted: bool,

    /// The time range covered by [`Self::times`].
    ///
    /// Not necessarily contiguous! Just the min and max value found in [`Self::times`].
    pub(crate) time_range: AbsoluteTimeRange,
}

/// Errors when deserializing/parsing/reading a column of time data.
#[derive(Debug, thiserror::Error)]
pub enum TimeColumnError {
    #[error("Time columns had nulls, but should be dense")]
    ContainsNulls,

    #[error("Unsupported data type : {0}")]
    UnsupportedDataType(arrow::datatypes::DataType),
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
        row_ids: FixedSizeBinaryArray,
        timelines: IntMap<TimelineName, TimeColumn>,
        components: ChunkComponents,
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
        timelines: IntMap<TimelineName, TimeColumn>,
        components: ChunkComponents,
    ) -> ChunkResult<Self> {
        re_tracing::profile_function!();
        let row_ids = RowId::arrow_from_slice(row_ids);
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
        timelines: IntMap<TimelineName, TimeColumn>,
        components: ChunkComponents,
    ) -> ChunkResult<Self> {
        let count = components
            .list_arrays()
            .next()
            .map_or(0, |list_array| list_array.len());

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
        row_ids: FixedSizeBinaryArray,
        components: ChunkComponents,
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
            row_ids: RowId::arrow_from_slice(&[]),
            timelines: Default::default(),
            components: Default::default(),
        }
    }

    /// Unconditionally inserts a [`SerializedComponentColumn`].
    ///
    /// Removes and replaces the column if it already exists.
    ///
    /// This will fail if the end result is malformed in any way -- see [`Self::sanity_check`].
    #[inline]
    pub fn add_component(
        &mut self,
        component_column: SerializedComponentColumn,
    ) -> ChunkResult<()> {
        self.components.insert(component_column);
        self.sanity_check()
    }

    /// Unconditionally inserts a [`TimeColumn`].
    ///
    /// Removes and replaces the column if it already exists.
    ///
    /// This will fail if the end result is malformed in any way -- see [`Self::sanity_check`].
    #[inline]
    pub fn add_timeline(&mut self, chunk_timeline: TimeColumn) -> ChunkResult<()> {
        self.timelines
            .insert(*chunk_timeline.timeline.name(), chunk_timeline);
        self.sanity_check()
    }
}

impl TimeColumn {
    /// Creates a new [`TimeColumn`].
    ///
    /// Iff you know for sure whether the data is already appropriately sorted or not, specify `is_sorted`.
    /// When left unspecified (`None`), it will be computed in O(n) time.
    ///
    /// For a row-oriented constructor, see [`Self::builder`].
    pub fn new(is_sorted: Option<bool>, timeline: Timeline, times: ArrowScalarBuffer<i64>) -> Self {
        re_tracing::profile_function_if!(1000 < times.len(), format!("{} times", times.len()));

        let time_slice = times.as_ref();

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
            AbsoluteTimeRange::new(min_time, max_time)
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
            AbsoluteTimeRange::new(min_time, max_time)
        };

        Self {
            timeline,
            times,
            is_sorted,
            time_range,
        }
    }

    /// Creates a new [`TimeColumn`] of sequence type.
    pub fn new_sequence(
        name: impl Into<re_log_types::TimelineName>,
        times: impl IntoIterator<Item = impl Into<i64>>,
    ) -> Self {
        let time_vec: Vec<_> = times.into_iter().map(|t| {
            let t = t.into();
            TimeInt::try_from(t)
                .unwrap_or_else(|_| {
                    re_log::error!(
                illegal_value = t,
                new_value = TimeInt::MIN.as_i64(),
                "TimeColumn::new_sequence() called with illegal value - clamped to minimum legal value"
            );
                    TimeInt::MIN
                })
                .as_i64()
        }).collect();

        Self::new(
            None,
            Timeline::new_sequence(name.into()),
            ArrowScalarBuffer::from(time_vec),
        )
    }

    /// Creates a new [`TimeColumn`] of duration type, in seconds.
    pub fn new_duration_secs(
        name: impl Into<re_log_types::TimelineName>,
        seconds: impl IntoIterator<Item = impl Into<f64>>,
    ) -> Self {
        let time_vec = seconds.into_iter().map(|seconds| {
            let seconds = seconds.into();
            let nanos = (1e9 * seconds).round();
            let clamped = NonMinI64::saturating_from_i64(nanos as i64);
            if clamped.get() as f64 != nanos {
                re_log::warn!(
                    illegal_value = nanos,
                    new_value = clamped.get(),
                    "TimeColumn::new_duration_secs() called with out-of-range value. Clamped to valid range."
                );
            }
            clamped.get()
        }).collect_vec();

        Self::new(
            None,
            Timeline::new(name, TimeType::DurationNs),
            ArrowScalarBuffer::from(time_vec),
        )
    }

    /// Creates a new [`TimeColumn`] of duration type, in seconds.
    pub fn new_timestamp_secs_since_epoch(
        name: impl Into<re_log_types::TimelineName>,
        seconds: impl IntoIterator<Item = impl Into<f64>>,
    ) -> Self {
        let time_vec = seconds.into_iter().map(|seconds| {
            let seconds = seconds.into();
            let nanos = (1e9 * seconds).round();
            let clamped = NonMinI64::saturating_from_i64(nanos as i64);
            if clamped.get() as f64 != nanos {
                re_log::warn!(
                    illegal_value = nanos,
                    new_value = clamped.get(),
                    "TimeColumn::new_timestamp_secs_since_epoch() called with out-of-range value. Clamped to valid range."
                );
            }
            clamped.get()
        }).collect_vec();

        Self::new(
            None,
            Timeline::new(name, TimeType::TimestampNs),
            ArrowScalarBuffer::from(time_vec),
        )
    }

    /// Creates a new [`TimeColumn`] measuring duration in nanoseconds.
    pub fn new_duration_nanos(
        name: impl Into<re_log_types::TimelineName>,
        nanos: impl IntoIterator<Item = impl Into<i64>>,
    ) -> Self {
        let time_vec = nanos
            .into_iter()
            .map(|nanos| {
                let nanos = nanos.into();
                NonMinI64::new(nanos)
                    .unwrap_or_else(|| {
                        re_log::error!(
                            illegal_value = nanos,
                            new_value = TimeInt::MIN.as_i64(),
                            "TimeColumn::new_duration_nanos() called with illegal value - clamped to minimum legal value"
                        );
                        NonMinI64::MIN
                    })
                    .get()
            })
            .collect_vec();

        Self::new(
            None,
            Timeline::new(name, TimeType::DurationNs),
            ArrowScalarBuffer::from(time_vec),
        )
    }

    /// Creates a new [`TimeColumn`] of timestamps, as nanoseconds since unix epoch.
    pub fn new_timestamp_nanos_since_epoch(
        name: impl Into<re_log_types::TimelineName>,
        nanos: impl IntoIterator<Item = impl Into<i64>>,
    ) -> Self {
        let time_vec = nanos
            .into_iter()
            .map(|nanos| {
                let nanos = nanos.into();
                NonMinI64::new(nanos)
                    .unwrap_or_else(|| {
                        re_log::error!(
                            illegal_value = nanos,
                            new_value = TimeInt::MIN.as_i64(),
                            "TimeColumn::new_timestamp_nanos_since_epoch() called with illegal value - clamped to minimum legal value"
                        );
                        NonMinI64::MIN
                    })
                    .get()
            })
            .collect_vec();

        Self::new(
            None,
            Timeline::new(name, TimeType::TimestampNs),
            ArrowScalarBuffer::from(time_vec),
        )
    }

    /// Parse the given [`ArrowArray`] as a time column.
    ///
    /// Results in an error if the array is of the wrong datatype, or if it contains nulls.
    pub fn read_array(array: &dyn ArrowArray) -> Result<ArrowScalarBuffer<i64>, TimeColumnError> {
        if array.null_count() > 0 {
            Err(TimeColumnError::ContainsNulls)
        } else {
            Self::read_nullable_array(array).map(|(times, _nulls)| times)
        }
    }

    /// Parse the given [`ArrowArray`] as a time column where null values are acceptable.
    ///
    /// Results in an error if the array is of the wrong datatype.
    pub fn read_nullable_array(
        array: &dyn ArrowArray,
    ) -> Result<(ArrowScalarBuffer<i64>, Option<ArrowNullBuffer>), TimeColumnError> {
        // Sequence timelines are i64, but time columns are nanoseconds (also as i64).
        if let Some(times) = array.downcast_array_ref::<arrow::array::Int64Array>() {
            Ok((times.values().clone(), times.nulls().cloned()))
        } else if let Some(times) =
            array.downcast_array_ref::<arrow::array::TimestampNanosecondArray>()
        {
            Ok((times.values().clone(), times.nulls().cloned()))
        } else if let Some(times) =
            array.downcast_array_ref::<arrow::array::Time64NanosecondArray>()
        {
            Ok((times.values().clone(), times.nulls().cloned()))
        } else if let Some(times) =
            array.downcast_array_ref::<arrow::array::DurationNanosecondArray>()
        {
            Ok((times.values().clone(), times.nulls().cloned()))
        } else {
            Err(TimeColumnError::UnsupportedDataType(
                array.data_type().clone(),
            ))
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

    #[inline]
    pub fn row_ids_array(&self) -> &FixedSizeBinaryArray {
        &self.row_ids
    }

    #[inline]
    pub fn row_ids_slice(&self) -> &[RowId] {
        RowId::slice_from_arrow(&self.row_ids)
    }

    /// All the [`RowId`] in this chunk.
    ///
    /// This could be in any order if this chunk is unsorted.
    #[inline]
    pub fn row_ids(&self) -> impl ExactSizeIterator<Item = RowId> + '_ {
        self.row_ids_slice().iter().copied()
    }

    /// Returns an iterator over the [`RowId`]s of a [`Chunk`], for a given component.
    ///
    /// This is different than [`Self::row_ids`]: it will only yield `RowId`s for rows at which
    /// there is data for the specified `component`.
    #[inline]
    pub fn component_row_ids(
        &self,
        component: ComponentIdentifier,
    ) -> impl Iterator<Item = RowId> + '_ + use<'_> {
        let Some(list_array) = self.components.get_array(component) else {
            return Either::Left(std::iter::empty());
        };

        let row_ids = self.row_ids();

        if let Some(validity) = list_array.nulls() {
            Either::Right(Either::Left(
                row_ids
                    .enumerate()
                    .filter_map(|(i, o)| validity.is_valid(i).then_some(o)),
            ))
        } else {
            Either::Right(Either::Right(row_ids))
        }
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

        let row_ids = self.row_ids_slice();

        #[expect(clippy::unwrap_used)] // checked above
        Some(if self.is_sorted() {
            (
                row_ids.first().copied().unwrap(),
                row_ids.last().copied().unwrap(),
            )
        } else {
            (
                row_ids.iter().min().copied().unwrap(),
                row_ids.iter().max().copied().unwrap(),
            )
        })
    }

    #[inline]
    pub fn is_static(&self) -> bool {
        self.timelines.is_empty()
    }

    #[inline]
    pub fn timelines(&self) -> &IntMap<TimelineName, TimeColumn> {
        &self.timelines
    }

    #[inline]
    pub fn components_identifiers(&self) -> impl Iterator<Item = ComponentIdentifier> + '_ {
        self.components.keys().copied()
    }

    #[inline]
    pub fn components(&self) -> &ChunkComponents {
        &self.components
    }
}

impl std::fmt::Display for Chunk {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let batch = self.to_record_batch().map_err(|err| {
            re_log::error_once!("couldn't display Chunk: {err}");
            std::fmt::Error
        })?;
        re_arrow_util::format_record_batch_with_width(&batch, f.width(), f.sign_minus()).fmt(f)
    }
}

impl TimeColumn {
    #[inline]
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.timeline.name()
    }

    #[inline]
    pub fn time_range(&self) -> AbsoluteTimeRange {
        self.time_range
    }

    #[inline]
    pub fn times_buffer(&self) -> &ArrowScalarBuffer<i64> {
        &self.times
    }

    /// Returns an array with the appropriate datatype.
    #[inline]
    pub fn times_array(&self) -> ArrowArrayRef {
        self.timeline.typ().make_arrow_array(self.times.clone())
    }

    /// All times in a time column are guaranteed not to have the value `i64::MIN`
    /// (which is reserved for static data).
    #[inline]
    pub fn times_raw(&self) -> &[i64] {
        self.times.as_ref()
    }

    /// All times in a time column are guaranteed not to have the value `i64::MIN`
    /// (which is reserved for static data).
    #[inline]
    pub fn times_nonmin(&self) -> impl DoubleEndedIterator<Item = NonMinI64> + '_ {
        self.times_raw()
            .iter()
            .copied()
            .map(NonMinI64::saturating_from_i64)
    }

    #[inline]
    pub fn times(&self) -> impl DoubleEndedIterator<Item = TimeInt> + '_ {
        self.times_raw().iter().copied().map(TimeInt::new_temporal)
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
    /// This is different from the time range covered by the [`TimeColumn`] as a whole
    /// because component columns are potentially sparse.
    ///
    /// This is crucial for indexing and queries to work properly.
    //
    // TODO(cmc): This needs to be stored in chunk metadata and transported across IPC.
    pub fn time_range_per_component(
        &self,
        components: &ChunkComponents,
    ) -> IntMap<ComponentIdentifier, AbsoluteTimeRange> {
        let times = self.times_raw();
        components
            .iter()
            .filter_map(|(component, column)| {
                if let Some(validity) = column.list_array.nulls() {
                    // Potentially sparse

                    if validity.is_empty() {
                        return None;
                    }

                    let is_dense = validity.null_count() == 0;
                    if is_dense {
                        return Some((*component, self.time_range));
                    }

                    let time_min = {
                        let mut valid_times = times
                            .iter()
                            .enumerate()
                            .filter(|(i, _time)| validity.is_valid(*i));
                        if times.is_sorted() {
                            valid_times.next()
                        } else {
                            valid_times.min_by_key(|(_i, time)| *time)
                        }
                        .map_or(TimeInt::MAX, |(_i, time)| TimeInt::new_temporal(*time))
                    };

                    let time_max = {
                        let mut valid_times_inv = times
                            .iter()
                            .enumerate()
                            .rev()
                            .filter(|(i, _time)| validity.is_valid(*i));
                        if times.is_sorted() {
                            valid_times_inv.next()
                        } else {
                            valid_times_inv.max_by_key(|(_i, time)| *time)
                        }
                        .map_or(TimeInt::MIN, |(_i, time)| TimeInt::new_temporal(*time))
                    };

                    Some((*component, AbsoluteTimeRange::new(time_min, time_max)))
                } else {
                    // Dense

                    Some((*component, self.time_range))
                }
            })
            .collect()
    }
}

impl re_byte_size::SizeBytes for Chunk {
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

impl re_byte_size::SizeBytes for TimeColumn {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            timeline,
            times,
            is_sorted,
            time_range,
        } = self;

        timeline.heap_size_bytes()
            + times.heap_size_bytes()
            + is_sorted.heap_size_bytes()
            + time_range.heap_size_bytes()
    }
}

// --- Sanity checks ---

impl Chunk {
    /// Returns an error if the Chunk's invariants are not upheld.
    ///
    /// Costly checks are only run in debug builds.
    #[track_caller]
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
            if *row_ids.data_type() != RowId::arrow_datatype() {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "RowId data has the wrong datatype: expected {} but got {} instead",
                        RowId::arrow_datatype(),
                        row_ids.data_type(),
                    ),
                });
            }

            #[expect(clippy::collapsible_if)] // readability
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
        for (timeline_name, time_column) in timelines {
            if time_column.times.len() != row_ids.len() {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "All timelines in a chunk must have the same number of timestamps, matching the number of row IDs. \
                         Found {} row IDs but {} timestamps for timeline '{timeline_name}'",
                        row_ids.len(),
                        time_column.times.len(),
                    ),
                });
            }

            time_column.sanity_check()?;
        }

        // Components

        for (component, column) in components.iter() {
            let SerializedComponentColumn {
                list_array,
                descriptor,
            } = column;

            if descriptor.component != *component {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "Component key & descriptor mismatch. Descriptor: {descriptor:?}. Key: {component:?}",
                    ),
                });
            }

            // Ensure that each cell is a list (we don't support mono-components yet).
            if let arrow::datatypes::DataType::List(_field) = list_array.data_type() {
                // We don't check `field.is_nullable()` here because we support both.
                // TODO(#6819): Remove support for inner nullability.
            } else {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "The inner array in a chunked component batch must be a list, got {:?}",
                        list_array.data_type(),
                    ),
                });
            }

            if list_array.len() != row_ids.len() {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "All component batches in a chunk must have the same number of rows, matching the number of row IDs. \
                             Found {} row IDs but {} rows for component batch {component}",
                        row_ids.len(),
                        list_array.len(),
                    ),
                });
            }

            let validity_is_empty = list_array
                .nulls()
                .is_some_and(|validity| validity.is_empty());
            if !self.is_empty() && validity_is_empty {
                return Err(ChunkError::Malformed {
                    reason: format!(
                        "All component batches in a chunk must contain at least one non-null entry.\
                             Found a completely empty column for {component}",
                    ),
                });
            }
        }

        Ok(())
    }
}

impl TimeColumn {
    /// Returns an error if the Chunk's invariants are not upheld.
    ///
    /// Costly checks are only run in debug builds.
    #[track_caller]
    pub fn sanity_check(&self) -> ChunkResult<()> {
        let Self {
            timeline: _,
            times,
            is_sorted,
            time_range,
        } = self;

        let times = times.as_ref();

        if cfg!(debug_assertions)
            && *is_sorted != times.windows(2).all(|times| times[0] <= times[1])
        {
            return Err(ChunkError::Malformed {
                reason: format!(
                    "Time column is marked as {}sorted but isn't: {times:?}",
                    if *is_sorted { "" } else { "un" },
                ),
            });
        }

        if cfg!(debug_assertions) {
            let is_tight_lower_bound = times.iter().any(|&time| time == time_range.min().as_i64());
            let is_tight_upper_bound = times.iter().any(|&time| time == time_range.max().as_i64());
            let is_tight_bound = is_tight_lower_bound && is_tight_upper_bound;

            if !self.is_empty() && !is_tight_bound {
                return Err(ChunkError::Malformed {
                    reason: "Time column's cached time range isn't a tight bound.".to_owned(),
                });
            }

            for &time in times {
                if time < time_range.min().as_i64() || time > time_range.max().as_i64() {
                    return Err(ChunkError::Malformed {
                        reason: format!(
                            "Time column's cached time range is wrong.\
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
