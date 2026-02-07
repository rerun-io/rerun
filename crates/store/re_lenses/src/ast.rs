//! Private module with the AST-like definitions of lenses.
//!
//! **Note**: Apart from high-level entry points (like [`Op`] and [`Lens`],
//! we should not leak these elements into the public API. This allows us to
//! evolve the definition of lenses over time, if requirements change.

use std::str::FromStr as _;

use arrow::array::{AsArray as _, Int64Array, ListArray};
use arrow::compute::take;
use arrow::datatypes::DataType;
use itertools::Either;
use nohash_hasher::IntMap;
use re_arrow_combinators::{Selector, Transform as _};
use re_arrow_combinators::{map, reshape};
use re_chunk::{
    ArrowArray as _, Chunk, ChunkId, ComponentIdentifier, EntityPath, TimeColumn, Timeline,
    TimelineName,
};
use re_log_types::{EntityPathFilter, TimeType};
use re_sdk_types::{ComponentDescriptor, SerializedComponentColumn};
use vec1::Vec1;

use crate::semantic;

use crate::LensError;
use crate::builder::LensBuilder;
use crate::op::{self, OpError};

pub struct InputColumn {
    pub entity_path_filter: EntityPathFilter,
    pub component: ComponentIdentifier,
}

/// Target entity path for lens outputs.
#[derive(Debug, Clone, Default)]
pub enum TargetEntity {
    /// Use the matched input entity path.
    #[default]
    SameAsInput,

    /// Use a specific entity path.
    Explicit(EntityPath),
}

/// A component output.
///
/// Depending on the context in which this output is used, the result from
/// applying the `ops` should be a list array (1:1) or a list array of list arrays (1:N).
#[derive(Debug)]
pub struct ComponentOutput {
    pub component_descr: ComponentDescriptor,
    pub ops: Vec<Op>,
}

/// A time extraction output.
#[derive(Debug)]
pub struct TimeOutput {
    pub timeline_name: TimelineName,
    pub timeline_type: TimeType,
    pub ops: Vec<Op>,
}

#[derive(Debug)]
/// Each input row produces exactly one output row (1:1 mapping).
///
/// Outputs inherit times from the input chunk.
pub struct OneToOne {
    pub target_entity: TargetEntity,

    /// Component columns that will be created.
    pub components: Vec1<ComponentOutput>,

    /// Time columns that will be created.
    pub times: Vec<TimeOutput>,
}

#[derive(Debug)]
/// Each input row produces multiple output rows (1:N flat-map).
///
/// Outputs inherit times from the input chunk.
pub struct OneToMany {
    pub target_entity: TargetEntity,

    /// Component columns that will be created.
    pub components: Vec1<ComponentOutput>,

    /// Time columns that will be created.
    pub times: Vec<TimeOutput>,
}

#[derive(Debug)]
/// Static lens: outputs have no timelines (timeless data).
///
/// In many cases, static lenses will omit the input column entirely.
pub struct Static {
    pub target_entity: TargetEntity,

    /// Component columns that will be created.
    pub components: Vec1<ComponentOutput>,
}

/// Determines how a lens transforms input rows to output rows.
#[derive(Debug)]
pub enum LensKind {
    Columns(OneToOne),
    ScatterColumns(OneToMany),
    StaticColumns(Static),
}

type CustomFn = Box<dyn Fn(&ListArray) -> Result<ListArray, OpError> + Sync + Send>;

/// Provides commonly used transformations of component columns.
///
/// Individual operations are wrapped to hide their implementation details.
#[non_exhaustive]
pub enum Op {
    /// Selector operation using jq-like syntax for navigating and transforming Arrow data.
    ///
    /// The selector query string is parsed at execution time.
    Selector(String),

    /// Converts binary arrays to list arrays of `u8`.
    BinaryToListUInt8,

    /// Efficiently casts a component to a new `DataType`.
    Cast(op::Cast),

    /// Converts video codec strings to Rerun `VideoCodec` enum values (as `u32`).
    StringToVideoCodecUInt32,

    /// Prepends a prefix to each string value.
    StringPrefix(String),

    /// Appends a suffix to each string value.
    StringSuffix(String),

    /// Converts timestamp structs with `seconds` and `nanos` fields to total nanoseconds.
    TimeSpecToNanos,

    /// A user-defined arbitrary function to convert a component column.
    Func(CustomFn),
}

impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Selector(query) => f.debug_tuple("Selector").field(query).finish(),
            Self::BinaryToListUInt8 => f.debug_struct("BinaryToListUInt8").finish(),
            Self::Cast(inner) => f.debug_tuple("Cast").field(inner).finish(),
            Self::StringToVideoCodecUInt32 => f.debug_struct("StringToVideoCodecUInt32").finish(),
            Self::StringPrefix(prefix) => f.debug_tuple("StringPrefix").field(prefix).finish(),
            Self::StringSuffix(suffix) => f.debug_tuple("StringSuffix").field(suffix).finish(),
            Self::TimeSpecToNanos => f.debug_struct("TimeSpecToNanos").finish(),
            Self::Func(_) => f.debug_tuple("Func").field(&"<function>").finish(),
        }
    }
}

impl From<&str> for Op {
    fn from(value: &str) -> Self {
        Self::Selector(value.to_owned())
    }
}

impl Op {
    /// Creates a selector operation from a query string.
    ///
    /// The selector uses jq-like syntax for navigating and transforming Arrow data.
    /// The query string is parsed at execution time.
    ///
    /// # Examples
    ///
    /// - `.field` - Access a field in a struct
    /// - `.parent.child` - Access nested fields
    /// - `.array[]` - Explode/flatten an array into multiple rows
    /// - `.array[].field` - Explode array and access a field in each element
    pub fn selector(query: impl Into<String>) -> Self {
        Self::Selector(query.into())
    }

    /// Converts binary arrays to list arrays of `u8`.
    pub fn binary_to_list_uint8() -> Self {
        Self::BinaryToListUInt8
    }

    /// Efficiently casts a component to a new `DataType`.
    pub fn cast(data_type: DataType) -> Self {
        Self::Cast(op::Cast {
            to_inner_type: data_type,
        })
    }

    /// Ignores any input and returns a constant `ListArray`.
    ///
    /// Commonly used with [`LensBuilder::output_static_columns`].
    /// When used in non-static columns this function will _not_ guarantee the correct amount of rows.
    pub fn constant(value: ListArray) -> Self {
        Self::func(move |_| Ok(value.clone()))
    }

    /// Converts video codec strings to Rerun `VideoCodec` enum values (as `u32`).
    pub fn string_to_video_codec() -> Self {
        Self::StringToVideoCodecUInt32
    }

    /// Prepends a prefix to each string value.
    pub fn string_prefix(prefix: impl Into<String>) -> Self {
        Self::StringPrefix(prefix.into())
    }

    /// Appends a suffix to each string value.
    pub fn string_suffix(suffix: impl Into<String>) -> Self {
        Self::StringSuffix(suffix.into())
    }

    /// Converts timestamp structs with `seconds` and `nanos` fields to total nanoseconds.
    pub fn time_spec_to_nanos() -> Self {
        Self::TimeSpecToNanos
    }

    /// A user-defined arbitrary function to convert a component column.
    pub fn func<F>(func: F) -> Self
    where
        F: for<'a> Fn(&'a ListArray) -> Result<ListArray, OpError> + Send + Sync + 'static,
    {
        Self::Func(Box::new(func))
    }
}

impl Op {
    fn call(&self, list_array: &ListArray) -> Result<ListArray, OpError> {
        match self {
            Self::Selector(query) => {
                let selector = Selector::from_str(query)?;
                selector.transform(list_array).map_err(Into::into)
            }
            Self::Cast(op) => op.call(list_array),
            Self::BinaryToListUInt8 => map::MapList::new(semantic::BinaryToListUInt8::<i32>::new())
                .transform(list_array)
                .map_err(Into::into),
            Self::StringToVideoCodecUInt32 => {
                map::MapList::new(semantic::StringToVideoCodecUInt32::default())
                    .transform(list_array)
                    .map_err(Into::into)
            }
            Self::StringPrefix(prefix) => map::MapList::new(map::StringPrefix::new(prefix.clone()))
                .transform(list_array)
                .map_err(Into::into),
            Self::StringSuffix(suffix) => map::MapList::new(map::StringSuffix::new(suffix.clone()))
                .transform(list_array)
                .map_err(Into::into),
            Self::TimeSpecToNanos => map::MapList::new(semantic::TimeSpecToNanos::default())
                .transform(list_array)
                .map_err(Into::into),
            Self::Func(func) => func(list_array),
        }
    }
}

/// A lens that transforms component data from one form to another.
///
/// Lenses allow you to extract, transform, and restructure component data. They
/// are applied to chunks that match the specified entity path filter and contain
/// the target component.
///
/// # Assumptions
///
/// Works on component columns within a chunk. Because what goes into a chunk
/// is non-deterministic, and dependent on the batcher, no assumptions should be
/// made for values across rows.
pub struct Lens {
    pub(crate) input: InputColumn,
    pub(crate) outputs: Vec<LensKind>,
}

impl Lens {
    /// Returns a new [`LensBuilder`] with the given input column.
    ///
    /// By default, creates a one-to-one (temporal) lens. Call `.with_static()` or `.with_to_many()`
    /// on the builder to switch to a different mode.
    pub fn for_input_column(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
    ) -> LensBuilder {
        LensBuilder::new(entity_path_filter, component)
    }

    /// Applies this lens and creates one or more chunks.
    fn apply(&self, chunk: &Chunk) -> impl Iterator<Item = Result<Chunk, PartialChunk>> {
        let found = chunk.components().get(self.input.component);

        // This means we drop chunks that belong to the same entity but don't have the component.
        let Some(column) = found else {
            return Either::Left(std::iter::empty());
        };

        Either::Right(self.outputs.iter().map(|output| match output {
            LensKind::Columns(one_to_one) => one_to_one.apply(chunk, column),
            LensKind::StaticColumns(static_columns) => static_columns.apply(chunk, column),
            LensKind::ScatterColumns(one_to_many) => one_to_many.apply(chunk, column),
        }))
    }
}

/// An optional [`Chunk`] that only contains the component and time columns that we were able to compute.
///
/// Also contains a list of contextualized errors that describe which columns failed.
#[derive(Debug)]
pub struct PartialChunk {
    /// [`Self`] is only used in an [`Result::Err`] variant.
    ///
    /// We therefore box the actual payload to keep the happy path optimized.
    inner: Box<PartialChunkInner>,
}

#[derive(Debug)]
struct PartialChunkInner {
    /// In some cases we might not be able to produce a chunk at all.
    chunk: Option<Chunk>,

    /// Collection of errors encountered while executing the Lens.
    errors: Vec<LensError>,
}

impl PartialChunk {
    /// Returns the partial chunk if any and consumes `self`.
    pub fn take(self) -> Option<Chunk> {
        self.inner.chunk
    }

    pub fn errors(&self) -> impl Iterator<Item = &LensError> {
        self.inner.errors.iter()
    }
}

fn apply_ops(initial: ListArray, ops: &[Op]) -> Result<ListArray, OpError> {
    ops.iter().try_fold(initial, |array, op| op.call(&array))
}

fn collect_output_components_iter<'a>(
    input: &'a SerializedComponentColumn,
    components: &'a [ComponentOutput],
) -> impl Iterator<Item = Result<(ComponentDescriptor, ListArray), LensError>> + 'a {
    components.iter().map(
        |output| match apply_ops(input.list_array.clone(), &output.ops) {
            Ok(list_array) => Ok((output.component_descr.clone(), list_array)),
            Err(source) => Err(LensError::ComponentOperationFailed {
                component: output.component_descr.component,
                source: Box::new(source),
            }),
        },
    )
}

fn collect_output_times_iter<'a>(
    input: &'a SerializedComponentColumn,
    timelines: &'a [TimeOutput],
) -> impl Iterator<Item = Result<(TimelineName, TimeType, ListArray), LensError>> + 'a {
    timelines.iter().map(
        |time| match apply_ops(input.list_array.clone(), &time.ops) {
            Ok(list_array) => Ok((time.timeline_name, time.timeline_type, list_array)),
            Err(source) => Err(LensError::TimeOperationFailed {
                timeline_name: time.timeline_name,
                source: Box::new(source),
            }),
        },
    )
}

/// Converts a time array to a time column.
///
/// Checks if the `list_array` values are [`arrow::array::Int64Array`] and if so, creates a [`re_chunk::TimeColumn`].
fn try_convert_time_column(
    timeline_name: TimelineName,
    timeline_type: TimeType,
    list_array: &ListArray,
) -> Result<(TimelineName, TimeColumn), LensError> {
    if let Some(time_vals) = list_array.values().as_any().downcast_ref::<Int64Array>() {
        let time_column = re_chunk::TimeColumn::new(
            None,
            Timeline::new(timeline_name, timeline_type),
            time_vals.values().clone(),
        );
        Ok((timeline_name, time_column))
    } else {
        Err(LensError::InvalidTimeColumn {
            timeline_name,
            actual_type: list_array.values().data_type().clone(),
        })
    }
}

fn resolve_entity_path<'a>(chunk: &'a Chunk, target_entity: &'a TargetEntity) -> &'a EntityPath {
    match target_entity {
        TargetEntity::SameAsInput => chunk.entity_path(),
        TargetEntity::Explicit(path) => path,
    }
}

/// Creates a chunk from the given components and timelines, handling errors appropriately.
///
/// Returns `Ok(chunk)` if successful with no errors, or `Err(PartialChunk)` if there were
/// errors during processing (with an optional chunk if creation succeeded despite errors).
fn finalize_chunk(
    entity_path: EntityPath,
    chunk_times: IntMap<TimelineName, TimeColumn>,
    component_results: re_chunk::ChunkComponents,
    mut errors: Vec<LensError>,
) -> Result<Chunk, PartialChunk> {
    match Chunk::from_auto_row_ids(ChunkId::new(), entity_path, chunk_times, component_results) {
        Ok(chunk) => {
            if errors.is_empty() {
                Ok(chunk)
            } else {
                Err(PartialChunk {
                    inner: Box::new(PartialChunkInner {
                        chunk: Some(chunk),
                        errors,
                    }),
                })
            }
        }
        Err(err) => {
            errors.push(err.into());
            Err(PartialChunk {
                inner: Box::new(PartialChunkInner {
                    chunk: None,
                    errors,
                }),
            })
        }
    }
}

impl OneToOne {
    /// Applies a one-to-one lens transformation where each input row produces exactly one output row.
    ///
    /// The output chunk inherits all timelines from the input chunk, with additional timelines
    /// extracted from the component data if specified. Component columns are transformed according
    /// to the provided operations.
    fn apply(
        &self,
        chunk: &Chunk,
        input: &SerializedComponentColumn,
    ) -> Result<Chunk, PartialChunk> {
        let entity_path = resolve_entity_path(chunk, &self.target_entity);

        let mut errors = Vec::new();

        // Collect successful components directly into ChunkComponents, accumulate errors
        let component_results: re_chunk::ChunkComponents =
            collect_output_components_iter(input, &self.components)
                .filter_map(|result| match result {
                    Ok(component) => Some(component),
                    Err(err) => {
                        errors.push(err);
                        None
                    }
                })
                .collect();

        // Inherit all existing time columns as-is (since row count doesn't change)
        let mut chunk_times = chunk.timelines().clone();

        // Collect successful time columns, accumulate errors
        chunk_times.extend(
            collect_output_times_iter(input, &self.times).filter_map(|result| match result {
                Ok((timeline_name, timeline_type, list_array)) => {
                    match try_convert_time_column(timeline_name, timeline_type, &list_array) {
                        Ok(time_col) => Some(time_col),
                        Err(err) => {
                            errors.push(err);
                            None
                        }
                    }
                }
                Err(err) => {
                    errors.push(err);
                    None
                }
            }),
        );

        finalize_chunk(entity_path.clone(), chunk_times, component_results, errors)
    }
}

impl Static {
    /// Applies a static lens transformation that produces timeless output data.
    ///
    /// The output chunk contains no time columns, only the transformed component columns.
    /// This is useful for metadata or other data that should not be associated with any timeline.
    fn apply(
        &self,
        chunk: &Chunk,
        input: &SerializedComponentColumn,
    ) -> Result<Chunk, PartialChunk> {
        let entity_path = resolve_entity_path(chunk, &self.target_entity);

        let mut errors = Vec::new();

        // Collect successful components directly into ChunkComponents, accumulate errors
        let component_results: re_chunk::ChunkComponents =
            collect_output_components_iter(input, &self.components)
                .filter_map(|result| match result {
                    Ok(component) => Some(component),
                    Err(err) => {
                        errors.push(err);
                        None
                    }
                })
                .collect();

        // TODO(grtlr): In case of static, should we enforce single rows (i.e. unit chunks)?
        finalize_chunk(
            entity_path.clone(),
            Default::default(),
            component_results,
            errors,
        )
    }
}

impl OneToMany {
    /// Applies a one-to-many lens transformation where each input row potentially produces multiple output rows.
    ///
    /// The output chunk inherits all time columns from the input chunk, with additional time columns
    /// extracted from the component data if specified. Component columns are transformed according
    /// to the provided operations.
    fn apply(
        &self,
        chunk: &Chunk,
        input: &SerializedComponentColumn,
    ) -> Result<Chunk, PartialChunk> {
        use arrow::array::UInt32Array;

        let entity_path = resolve_entity_path(chunk, &self.target_entity);

        let mut errors = Vec::new();

        let mut output_components =
            collect_output_components_iter(input, &self.components).peekable();

        // Peek at the first component to establish the scatter pattern (how many output rows
        // each input row produces). All components must have the same outer list structure.
        // We use .peek() instead of consuming the iterator so we can still process all
        // components (including this first one) later.
        let reference_array = match output_components.peek() {
            Some(Ok((_descr, reference_array))) => reference_array,
            Some(Err(_)) => {
                // If the first component failed, collect all errors and return
                errors.extend(output_components.filter_map(|r| r.err()));
                return Err(PartialChunk {
                    inner: Box::new(PartialChunkInner {
                        chunk: None,
                        errors,
                    }),
                });
            }
            None => {
                return Err(PartialChunk {
                    inner: Box::new(PartialChunkInner {
                        chunk: None,
                        errors: vec![LensError::NoOutputColumnsProduced {
                            input_entity: chunk.entity_path().clone(),
                            input_component: input.descriptor.component,
                            target_entity: entity_path.clone(),
                        }],
                    }),
                });
            }
        };

        // Build scatter indices: tracks which input row each output row came from
        // Example: [0, 0, 0, 1, 2] means rows 0-2 from input 0, row 3 from input 1, row 4 from input 2
        let mut scatter_indices = Vec::new();
        let offsets = reference_array.value_offsets();

        for (row_idx, window) in offsets.windows(2).enumerate() {
            let start = window[0];
            let end = window[1];
            let count = end - start;

            if reference_array.is_null(row_idx) || count == 0 {
                // Null or empty list produces one output row
                scatter_indices.push(row_idx as u32);
            } else {
                // Each element produces one output row
                for _ in 0..count {
                    scatter_indices.push(row_idx as u32);
                }
            }
        }

        let scatter_indices_array = UInt32Array::from(scatter_indices);

        // Replicate all existing time values using scatter indices.
        let mut chunk_times: IntMap<TimelineName, TimeColumn> = Default::default();
        for (timeline_name, time_column) in chunk.timelines() {
            let time_values = time_column.times_raw();
            let time_values_array = Int64Array::from(time_values.to_vec());

            // `arrow::compute::take` is fine to use in this context, because we want to allow nullability.
            #[expect(clippy::disallowed_methods)]
            match take(&time_values_array, &scatter_indices_array, None) {
                Ok(scattered) => {
                    let scattered_i64 = scattered.as_primitive::<arrow::datatypes::Int64Type>();
                    let new_time_column = re_chunk::TimeColumn::new(
                        None,
                        *time_column.timeline(),
                        scattered_i64.values().clone(),
                    );
                    chunk_times.insert(*timeline_name, new_time_column);
                }
                Err(source) => {
                    errors.push(LensError::ScatterExistingTimeFailed {
                        timeline_name: *timeline_name,
                        source,
                    });
                }
            }
        }

        // Explode all output time columns and collect errors
        chunk_times.extend(
            collect_output_times_iter(input, &self.times).filter_map(|result| match result {
                Ok((timeline_name, timeline_type, list_array)) => {
                    match reshape::Explode.transform(&list_array) {
                        Ok(exploded) => {
                            match try_convert_time_column(timeline_name, timeline_type, &exploded) {
                                Ok(time_col) => Some(time_col),
                                Err(err) => {
                                    errors.push(err);
                                    None
                                }
                            }
                        }
                        Err(err) => {
                            errors.push(LensError::TimeOperationFailed {
                                timeline_name,
                                source: Box::new(err.into()),
                            });
                            None
                        }
                    }
                }
                Err(err) => {
                    errors.push(err);
                    None
                }
            }),
        );

        // Explode all component outputs and collect errors
        let chunk_components: re_chunk::ChunkComponents = output_components
            .filter_map(|result| match result {
                Ok((component_descr, list_array)) => {
                    match reshape::Explode.transform(&list_array) {
                        Ok(exploded) => {
                            Some(SerializedComponentColumn::new(exploded, component_descr))
                        }
                        Err(err) => {
                            errors.push(LensError::ComponentOperationFailed {
                                component: component_descr.component,
                                source: Box::new(err.into()),
                            });
                            None
                        }
                    }
                }
                Err(err) => {
                    errors.push(err);
                    None
                }
            })
            .collect();

        // Verify that all columns have the same length happens during chunk creation.
        finalize_chunk(entity_path.clone(), chunk_times, chunk_components, errors)
    }
}

/// Controls how data is processed when applying lenses.
///
/// This determines what happens to logged data when lenses are applied, particularly
/// how unmatched original data is handled.
#[derive(Copy, Clone)]
pub enum OutputMode {
    /// Forward both the transformed data from matching lenses and the original data.
    ///
    /// Use this when you want to preserve all original data alongside transformations.
    ForwardAll,

    /// Forward transformed data if lenses match, otherwise forward the original data unchanged.
    ///
    /// Use this when you want to transform matching data but ensure unmatched data isn't dropped.
    ForwardUnmatched,

    /// Only forward transformed data, drop data that doesn't match any lens.
    ///
    /// Use this when you want a pure transformation pipeline where only explicitly transformed
    /// data should be output.
    DropUnmatched,
}

/// A collection that holds multiple lenses and applies them to chunks.
///
/// This can hold multiple lenses that match different entity paths and components.
/// When a chunk is processed, all relevant lenses (those whose entity path filters match
/// the chunk's entity path) are applied.
pub struct Lenses {
    lenses: Vec<Lens>,
    mode: OutputMode,
}

impl Lenses {
    /// Creates a new lens collection with the specified mode.
    pub fn new(mode: OutputMode) -> Self {
        Self {
            lenses: Default::default(),
            mode,
        }
    }

    /// Adds a lens to this collection.
    pub fn add_lens(&mut self, lens: Lens) {
        self.lenses.push(lens);
    }

    /// Adds a lens to this collection.
    pub fn set_output_mode(&mut self, mode: OutputMode) {
        self.mode = mode;
    }

    fn relevant(&self, chunk: &Chunk) -> impl Iterator<Item = &Lens> {
        self.lenses.iter().filter(|lens| {
            lens.input
                .entity_path_filter
                .clone()
                .resolve_without_substitutions()
                .matches(chunk.entity_path())
                && chunk.components().contains_component(lens.input.component)
        })
    }

    /// Applies all relevant lenses and returns the results.
    ///
    /// The behavior depends on the configured [`OutputMode`]:
    /// - [`OutputMode::ForwardAll`]: Returns both transformed and original data
    /// - [`OutputMode::ForwardUnmatched`]: Returns transformed data if lenses match, otherwise original data
    /// - [`OutputMode::DropUnmatched`]: Returns only transformed data, drops unmatched data
    pub fn apply<'a>(
        &'a self,
        chunk: &'a Chunk,
    ) -> impl Iterator<Item = Result<Chunk, PartialChunk>> + 'a {
        match self.mode {
            OutputMode::ForwardAll => {
                // Apply all relevant lenses and also forward the original chunk
                let chunk_clone = chunk.clone();
                Either::Left(
                    self.relevant(chunk)
                        .flat_map(|lens| lens.apply(chunk))
                        .chain(std::iter::once(Ok(chunk_clone))),
                )
            }
            OutputMode::ForwardUnmatched => {
                // Apply relevant lenses if any exist, otherwise forward the original chunk
                let chunk_clone = chunk.clone();
                let mut relevant_lenses = self.relevant(chunk).peekable();
                let has_relevant = relevant_lenses.peek().is_some();

                Either::Right(Either::Left(
                    relevant_lenses
                        .flat_map(|lens| lens.apply(chunk))
                        .chain((!has_relevant).then_some(Ok(chunk_clone))),
                ))
            }
            OutputMode::DropUnmatched => Either::Right(Either::Right(
                self.relevant(chunk).flat_map(|lens| lens.apply(chunk)),
            )),
        }
    }
}
