//! Private module with the AST-like definitions of lenses.
//!
//! **Note**: Apart from high-level entry points (like [`Lens`]),
//! we should not leak these elements into the public API. This allows us to
//! evolve the definition of lenses over time, if requirements change.

use std::collections::BTreeMap;

use crate::combinators::{Explode, Transform as _};
use crate::{DynExpr, LensRuntimeError, Selector};
use arrow::array::{AsArray as _, Int64Array, ListArray};
use arrow::compute::take;
use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk::{
    ArrowArray as _, Chunk, ChunkId, ComponentIdentifier, EntityPath, TimeColumn, Timeline,
    TimelineName,
};
use re_log_types::{ResolvedEntityPathFilter, TimeType};
use re_sdk_types::{ComponentDescriptor, SerializedComponentColumn};
use vec1::Vec1;

use crate::builder::LensBuilder;

type ChunkTimelines = IntMap<TimelineName, TimeColumn>;

/// A component output.
///
/// Depending on the context in which this output is used, the result from
/// applying the transform should be a list array (1:1) or a list array of list arrays (1:N).
#[derive(Clone, Debug)]
pub struct ComponentOutput {
    pub component_descr: ComponentDescriptor,
    pub selector: Selector<DynExpr>,
}

/// A time extraction output.
#[derive(Clone, Debug)]
pub struct TimeOutput {
    pub timeline_name: TimelineName,
    pub timeline_type: TimeType,
    pub selector: Selector<DynExpr>,
}

#[derive(Clone)]
pub struct LensOutput {
    /// Component columns that will be created.
    pub output_components: Vec1<ComponentOutput>,

    /// Time columns that will be created.
    pub output_timelines: Vec<TimeOutput>,
}

impl LensOutput {
    fn apply(
        &self,
        scatter: bool,
        target_entity: &EntityPath,
        timelines: &ChunkTimelines,
        input: &SerializedComponentColumn,
    ) -> Result<Chunk, PartialChunk> {
        if scatter {
            apply_one_to_many(
                target_entity,
                timelines,
                &self.output_timelines,
                &self.output_components,
                input,
            )
        } else {
            apply_one_to_one(
                target_entity,
                timelines,
                &self.output_timelines,
                &self.output_components,
                input,
            )
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
/// There can be at most one set of output columns per target entity within a lens.
///
/// Works on component columns within a chunk. Because what goes into a chunk
/// is non-deterministic, and dependent on the batcher, no assumptions should be
/// made for values across rows.
#[derive(Clone)]
pub struct Lens {
    pub(crate) input: ComponentIdentifier,

    /// When `true`, use 1:N row mapping (scatter/explode lists).
    /// When `false`, use 1:1 row mapping.
    pub(crate) scatter: bool,

    /// Output for the same entity as the input.
    pub(crate) same_entity_output: Option<LensOutput>,

    /// Outputs keyed by explicit target entity path.
    pub(crate) entity_outputs: BTreeMap<EntityPath, LensOutput>,
}

impl Lens {
    /// Returns a new [`LensBuilder`] for the given input component column.
    ///
    /// By default, creates a one-to-one (temporal) lens. Call `.with_static()` or `.with_to_many()`
    /// on the builder to switch to a different mode.
    pub fn for_input_column(component: impl Into<ComponentIdentifier>) -> LensBuilder {
        LensBuilder::new(component)
    }

    /// Applies this lens and creates one or more chunks.
    fn apply<'a>(
        &'a self,
        original_entity: &'a EntityPath,
        timelines: &'a ChunkTimelines,
        input: &'a SerializedComponentColumn,
    ) -> impl Iterator<Item = Result<Chunk, PartialChunk>> + 'a {
        let scatter = self.scatter;
        self.same_entity_output
            .iter()
            .map(move |output| output.apply(scatter, original_entity, timelines, input))
            .chain(
                self.entity_outputs
                    .iter()
                    .map(move |(path, output)| output.apply(scatter, path, timelines, input)),
            )
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
    errors: Vec<LensRuntimeError>,
}

impl PartialChunk {
    /// Returns the partial chunk if any and consumes `self`.
    pub fn take(self) -> Option<Chunk> {
        self.inner.chunk
    }

    pub fn errors(&self) -> impl Iterator<Item = &LensRuntimeError> {
        self.inner.errors.iter()
    }
}

fn collect_components_iter<'a>(
    input: &'a SerializedComponentColumn,
    components: &'a [ComponentOutput],
    target_entity: &'a EntityPath,
) -> impl Iterator<Item = Result<(ComponentDescriptor, ListArray), LensRuntimeError>> + 'a {
    components.iter().filter_map(move |output| {
        match output.selector.execute_per_row(&input.list_array) {
            Ok(Some(list_array)) => Some(Ok((output.component_descr.clone(), list_array))),
            Ok(None) => {
                re_log::debug_once!(
                    "Lens suppressed for `{target_entity}` component `{}`",
                    output.component_descr.component
                );
                None
            }
            Err(source) => Some(Err(LensRuntimeError::ComponentOperationFailed {
                target_entity: target_entity.clone(),
                input_component: input.descriptor.component,
                component: output.component_descr.component,
                source: Box::new(source),
            })),
        }
    })
}

fn collect_output_times_iter<'a>(
    input: &'a SerializedComponentColumn,
    timelines: &'a [TimeOutput],
    target_entity: &'a EntityPath,
) -> impl Iterator<Item = Result<(TimelineName, TimeType, ListArray), LensRuntimeError>> + 'a {
    timelines.iter().filter_map(move |time| {
        match time.selector.execute_per_row(&input.list_array) {
            Ok(Some(list_array)) => Some(Ok((time.timeline_name, time.timeline_type, list_array))),
            Ok(None) => {
                re_log::debug_once!(
                    "Lens suppressed for `{target_entity}` timeline `{}`",
                    time.timeline_name,
                );
                None
            }
            Err(source) => Some(Err(LensRuntimeError::TimeOperationFailed {
                target_entity: target_entity.clone(),
                input_component: input.descriptor.component,
                timeline_name: time.timeline_name,
                source: Box::new(source),
            })),
        }
    })
}

/// Converts a time array to a time column.
///
/// Checks if the `list_array` values are [`arrow::array::Int64Array`] and if so, creates a [`re_chunk::TimeColumn`].
fn try_convert_time_column(
    timeline_name: TimelineName,
    timeline_type: TimeType,
    list_array: &ListArray,
) -> Result<(TimelineName, TimeColumn), LensRuntimeError> {
    if let Some(time_vals) = list_array.values().as_any().downcast_ref::<Int64Array>() {
        let time_column = re_chunk::TimeColumn::new(
            None,
            Timeline::new(timeline_name, timeline_type),
            time_vals.values().clone(),
        );
        Ok((timeline_name, time_column))
    } else {
        Err(LensRuntimeError::InvalidTimeColumn {
            timeline_name,
            actual_type: list_array.values().data_type().clone(),
        })
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
    mut errors: Vec<LensRuntimeError>,
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

/// Applies a one-to-one lens transformation where each input row produces exactly one output row.
///
/// The output chunk inherits all timelines from the input chunk, with additional timelines
/// extracted from the component data if specified. Component columns are transformed according
/// to the provided operations.
fn apply_one_to_one(
    target_entity: &EntityPath,
    original_timelines: &ChunkTimelines,
    timelines: &[TimeOutput],
    components: &[ComponentOutput],
    input: &SerializedComponentColumn,
) -> Result<Chunk, PartialChunk> {
    let mut errors = Vec::new();

    let mut component_results = re_chunk::ChunkComponents::default();

    // Collect successful components directly into ChunkComponents, accumulate errors.
    for result in collect_components_iter(input, components, target_entity) {
        match result {
            Ok((component_descr, list_array)) => {
                component_results
                    .insert(SerializedComponentColumn::new(list_array, component_descr));
            }
            Err(err) => errors.push(err),
        }
    }

    // Inherit all existing time columns as-is (since row count doesn't change)
    let mut chunk_times = original_timelines.clone();

    // Collect successful time columns, accumulate errors
    chunk_times.extend(
        collect_output_times_iter(input, timelines, target_entity).filter_map(
            |result| match result {
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
            },
        ),
    );

    finalize_chunk(
        target_entity.clone(),
        chunk_times,
        component_results,
        errors,
    )
}

/// Applies a one-to-many lens transformation where each input row potentially produces multiple output rows.
///
/// The output chunk inherits all time columns from the input chunk, with additional time columns
/// extracted from the component data if specified. Component columns are transformed according
/// to the provided operations.
fn apply_one_to_many(
    target_entity: &EntityPath,
    original_timelines: &ChunkTimelines,
    timelines: &[TimeOutput],
    components: &[ComponentOutput],
    input: &SerializedComponentColumn,
) -> Result<Chunk, PartialChunk> {
    use arrow::array::UInt32Array;

    let mut errors = Vec::new();

    let mut components = collect_components_iter(input, components, target_entity).peekable();

    // Peek at the first component to establish the scatter pattern (how many output rows
    // each input row produces). All components must have the same outer list structure.
    // We use .peek() instead of consuming the iterator so we can still process all
    // components (including this first one) later.
    let reference_array = match components.peek() {
        Some(Ok((_descr, reference_array))) => reference_array,
        Some(Err(_)) => {
            // If the first component failed, collect all errors and return
            errors.extend(components.filter_map(|r| r.err()));
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
                    errors: vec![LensRuntimeError::NoOutputColumnsProduced {
                        input_component: input.descriptor.component,
                        target_entity: target_entity.clone(),
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
    for (timeline_name, time_column) in original_timelines {
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
                errors.push(LensRuntimeError::ScatterExistingTimeFailed {
                    timeline_name: *timeline_name,
                    source,
                });
            }
        }
    }

    // Explode all output time columns and collect errors
    chunk_times.extend(
        collect_output_times_iter(input, timelines, target_entity).filter_map(
            |result| match result {
                Ok((timeline_name, timeline_type, list_array)) => {
                    match Explode.transform(&list_array) {
                        Ok(Some(exploded)) => {
                            match try_convert_time_column(timeline_name, timeline_type, &exploded) {
                                Ok(time_col) => Some(time_col),
                                Err(err) => {
                                    errors.push(err);
                                    None
                                }
                            }
                        }
                        Ok(None) => None,
                        Err(err) => {
                            errors.push(LensRuntimeError::TimeOperationFailed {
                                target_entity: target_entity.clone(),
                                input_component: input.descriptor.component,
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
            },
        ),
    );

    let mut chunk_components = re_chunk::ChunkComponents::default();

    for result in components {
        match result {
            Ok((component_descr, list_array)) => match Explode.transform(&list_array) {
                Ok(Some(exploded)) => {
                    chunk_components
                        .insert(SerializedComponentColumn::new(exploded, component_descr));
                }
                Ok(None) => {}
                Err(err) => {
                    errors.push(LensRuntimeError::ComponentOperationFailed {
                        target_entity: target_entity.clone(),
                        input_component: input.descriptor.component,
                        component: component_descr.component,
                        source: Box::new(err.into()),
                    });
                }
            },
            Err(err) => errors.push(err),
        }
    }

    // Verify that all columns have the same length happens during chunk creation.
    finalize_chunk(target_entity.clone(), chunk_times, chunk_components, errors)
}

/// Controls how data is processed when applying lenses.
///
/// This determines what happens to columns when lenses are applied, particularly
/// how unmatched original columns are handled.
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
/// When a chunk is processed, all relevant lenses (those whose input component
/// matches a component in the chunk) are applied.
///
/// Each lens is paired with a [`ResolvedEntityPathFilter`] to control which
/// entity paths it applies to.
#[derive(Clone)]
pub struct Lenses {
    lenses: Vec<(ResolvedEntityPathFilter, Lens)>,
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

    /// Adds a lens that applies to all entity paths.
    pub fn add_lens(mut self, lens: Lens) -> Self {
        self.lenses.push((
            re_log_types::EntityPathFilter::all().resolve_without_substitutions(),
            lens,
        ));
        self
    }

    /// Adds a lens with an entity path filter.
    ///
    /// The lens will only be applied to chunks whose entity path matches the filter.
    pub fn add_lens_with_filter(
        mut self,
        filter: re_log_types::EntityPathFilter,
        lens: Lens,
    ) -> Self {
        self.lenses
            .push((filter.resolve_without_substitutions(), lens));
        self
    }

    /// Sets the output mode for this collection.
    pub fn set_output_mode(&mut self, mode: OutputMode) {
        self.mode = mode;
    }

    fn relevant_lenses(&self, chunk: &Chunk) -> impl Iterator<Item = &Lens> {
        let entity_path = chunk.entity_path();
        self.lenses
            .iter()
            .filter(|(filter, lens)| {
                filter.matches(entity_path) && chunk.components().contains_component(lens.input)
            })
            .map(|(_, lens)| lens)
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
        let prefix: Option<Chunk> = match self.mode {
            OutputMode::ForwardAll => Some(chunk.clone()),
            OutputMode::ForwardUnmatched => {
                let relevant_components = self
                    .relevant_lenses(chunk)
                    .map(|lens| lens.input)
                    .unique()
                    .collect::<Vec<_>>();
                let untouched = chunk.components_dropped(&relevant_components);
                (untouched.num_components() > 0).then_some(untouched)
            }
            OutputMode::DropUnmatched => None,
        };

        prefix.into_iter().map(Ok).chain(
            self.relevant_lenses(chunk)
                .filter_map(|lens| {
                    let component = chunk.components().get(lens.input)?;
                    Some(lens.apply(chunk.entity_path(), chunk.timelines(), component))
                })
                .flatten(),
        )
    }
}
