//! Execution of a lens [`Plan`] against a chunk.

use arrow::array::{AsArray as _, Int64Array, ListArray, UInt32Array};
use arrow::compute::take;
use re_chunk::{ArrowArray as _, Chunk, ChunkComponents, ChunkId};
use re_log_types::TimeType;
use re_sdk_types::{ComponentDescriptor, SerializedComponentColumn};

use crate::ast::{ComponentOutput, Lenses, Rows, TimeOutput};
use crate::combinators::{Explode, Transform as _};
use crate::error::{LensError, LensRuntimeError};

use crate::plan::{ChunkTimelines, DeriveWork, Plan};

fn output_components_iter<'a>(
    input: &'a SerializedComponentColumn,
    components: &'a [ComponentOutput],
    target_entity: &'a re_chunk::EntityPath,
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

fn output_timelines_iter<'a>(
    input: &'a SerializedComponentColumn,
    timelines: &'a [TimeOutput],
    target_entity: &'a re_chunk::EntityPath,
) -> impl Iterator<Item = Result<(re_chunk::TimelineName, TimeType, ListArray), LensRuntimeError>> + 'a
{
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
fn try_convert_time_column(
    timeline_name: re_chunk::TimelineName,
    timeline_type: TimeType,
    list_array: &ListArray,
) -> Result<(re_chunk::TimelineName, re_chunk::TimeColumn), LensRuntimeError> {
    if let Some(time_vals) = list_array.values().as_any().downcast_ref::<Int64Array>() {
        let time_column = re_chunk::TimeColumn::new(
            None,
            re_chunk::Timeline::new(timeline_name, timeline_type),
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

/// Builds a [`Chunk`] with auto-generated row IDs, wrapping construction
/// errors and partial results into [`LensError`].
fn finalize_chunk(
    entity_path: re_chunk::EntityPath,
    chunk_times: ChunkTimelines,
    components: ChunkComponents,
    mut errors: Vec<LensRuntimeError>,
) -> Result<Chunk, LensError> {
    match Chunk::from_auto_row_ids(ChunkId::new(), entity_path, chunk_times, components) {
        Ok(chunk) if errors.is_empty() => Ok(chunk),
        Ok(chunk) => Err(LensError::with_partial_chunk(chunk, errors)),
        Err(err) => {
            errors.push(err.into());
            Err(LensError::new(None, errors))
        }
    }
}

/// Applies a one-to-one lens transformation (each input row -> exactly one output row).
fn apply_one_to_one(work: &DeriveWork<'_>) -> Result<Chunk, LensError> {
    let mut errors = Vec::new();

    let mut component_results = re_chunk::ChunkComponents::default();

    for result in output_components_iter(work.input, work.components, work.target_entity) {
        match result {
            Ok((component_descr, list_array)) => {
                component_results
                    .insert(SerializedComponentColumn::new(list_array, component_descr));
            }
            Err(err) => errors.push(err),
        }
    }

    let mut chunk_times = work.original_timelines.clone();

    chunk_times.extend(
        output_timelines_iter(work.input, work.timelines, work.target_entity).filter_map(
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
        work.target_entity.clone(),
        chunk_times,
        component_results,
        errors,
    )
}

/// Computes scatter indices from a list array's offsets.
///
/// For each row: if null or empty, one index is emitted (preserving the row).
/// Otherwise, `count` copies of the row index are emitted (one per inner element).
fn compute_scatter_indices(reference: &ListArray) -> UInt32Array {
    use arrow::array::UInt32Array;

    let offsets = reference.value_offsets();
    let mut indices = Vec::new();

    for (row_idx, window) in offsets.windows(2).enumerate() {
        let count = window[1] - window[0];
        if reference.is_null(row_idx) || count == 0 {
            indices.push(row_idx as u32);
        } else {
            for _ in 0..count {
                indices.push(row_idx as u32);
            }
        }
    }

    UInt32Array::from(indices)
}

/// Replicates existing timeline columns according to scatter indices.
fn scatter_existing_timelines(
    original_timelines: &ChunkTimelines,
    scatter_indices: &UInt32Array,
    errors: &mut Vec<LensRuntimeError>,
) -> ChunkTimelines {
    let mut chunk_times: ChunkTimelines = Default::default();

    for (timeline_name, time_column) in original_timelines {
        let time_values = time_column.times_raw();
        let time_values_array = Int64Array::from(time_values.to_vec());

        // `take` is generally disallowed because callers often ignore null semantics,
        // but here we need exact index-based replication including nulls.
        #[expect(clippy::disallowed_methods)]
        match take(&time_values_array, scatter_indices, None) {
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

    chunk_times
}

/// Applies a one-to-many lens transformation (each input row -> potentially multiple output rows).
fn apply_one_to_many(work: &DeriveWork<'_>) -> Result<Chunk, LensError> {
    let mut errors = Vec::new();

    let mut components =
        output_components_iter(work.input, work.components, work.target_entity).peekable();

    let reference_array = match components.peek() {
        Some(Ok((_descr, reference_array))) => reference_array,
        Some(Err(_)) => {
            errors.extend(components.filter_map(|r| r.err()));
            return Err(LensError::new(None, errors));
        }
        None => {
            return Err(LensError::new(
                None,
                vec![LensRuntimeError::NoOutputColumnsProduced {
                    input_component: work.input.descriptor.component,
                    target_entity: work.target_entity.clone(),
                }],
            ));
        }
    };

    let scatter_indices_array = compute_scatter_indices(reference_array);
    let expected_rows = scatter_indices_array.len();

    let mut chunk_times =
        scatter_existing_timelines(work.original_timelines, &scatter_indices_array, &mut errors);

    chunk_times.extend(
        output_timelines_iter(work.input, work.timelines, work.target_entity).filter_map(
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
                                target_entity: work.target_entity.clone(),
                                input_component: work.input.descriptor.component,
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
                    if exploded.len() != expected_rows {
                        errors.push(LensRuntimeError::InconsistentOutputRows {
                            target_entity: work.target_entity.clone(),
                            component: component_descr.component,
                            expected: expected_rows,
                            actual: exploded.len(),
                        });
                    } else {
                        chunk_components
                            .insert(SerializedComponentColumn::new(exploded, component_descr));
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    errors.push(LensRuntimeError::ComponentOperationFailed {
                        target_entity: work.target_entity.clone(),
                        input_component: work.input.descriptor.component,
                        component: component_descr.component,
                        source: Box::new(err.into()),
                    });
                }
            },
            Err(err) => errors.push(err),
        }
    }

    finalize_chunk(
        work.target_entity.clone(),
        chunk_times,
        chunk_components,
        errors,
    )
}

/// Plans and executes relevant lenses against a chunk.
pub fn execute<'a>(
    lenses: &'a Lenses,
    chunk: &'a Chunk,
) -> impl Iterator<Item = Result<Chunk, LensError>> + 'a {
    let Plan {
        mutate_work,
        merge_work,
        derive_work,
        forward_columns,
        errors: plan_errors,
    } = crate::plan::plan(lenses, chunk);

    // --- Build prefix ---

    let has_modifications = !mutate_work.is_empty() || !merge_work.is_empty();

    let prefix: Option<Result<Chunk, LensError>> = if !has_modifications {
        let p: Option<Chunk> = if forward_columns.len() == chunk.components().len() {
            Some(chunk.clone())
        } else if forward_columns.is_empty() {
            None
        } else {
            let to_drop: Vec<_> = chunk
                .components()
                .keys()
                .copied()
                .filter(|id| !forward_columns.contains(id))
                .collect();
            let dropped = chunk.components_dropped(&to_drop);
            (!dropped.components().is_empty()).then_some(dropped)
        };
        if plan_errors.is_empty() {
            p.map(Ok)
        } else {
            Some(match p {
                Some(chunk) => Err(LensError::with_partial_chunk(chunk, plan_errors)),
                None => Err(LensError::new(None, plan_errors)),
            })
        }
    } else {
        let entity_path = chunk.entity_path();
        let timelines = chunk.timelines();
        let mut components = chunk.components().clone();
        let mut errors = plan_errors;

        // Drop columns that shouldn't be forwarded.
        components.retain(|id, _| forward_columns.contains(id));

        // Apply mutate modifications in-place.
        for (id, work) in &mutate_work {
            let Some(col) = components.get(*id) else {
                continue;
            };
            match work.selector.execute_per_row(&col.list_array) {
                Ok(Some(list_array)) => {
                    components.insert(SerializedComponentColumn::new(
                        list_array,
                        col.descriptor.clone(),
                    ));
                }
                Ok(None) => {
                    re_log::debug_once!(
                        "Mutate lens suppressed for `{entity_path}` component `{id}`",
                    );
                    components.remove(id);
                }
                Err(source) => {
                    errors.push(LensRuntimeError::ComponentOperationFailed {
                        target_entity: entity_path.clone(),
                        input_component: *id,
                        component: *id,
                        source: Box::new(source),
                    });
                }
            }
        }

        // Append merged same-entity derive outputs.
        for work in &merge_work {
            for result in output_components_iter(work.input, work.components, entity_path) {
                match result {
                    Ok((descr, list_array)) => {
                        components.insert(SerializedComponentColumn::new(list_array, descr));
                    }
                    Err(err) => errors.push(err),
                }
            }
        }

        if components.is_empty() && errors.is_empty() {
            None
        } else {
            let regenerate_row_ids = mutate_work.values().any(|w| !w.keep_row_ids);
            Some(if regenerate_row_ids {
                finalize_chunk(entity_path.clone(), timelines.clone(), components, errors)
            } else {
                match Chunk::new(
                    ChunkId::new(),
                    entity_path.clone(),
                    Some(chunk.is_sorted()),
                    chunk.row_ids_array().clone(),
                    timelines.clone(),
                    components,
                ) {
                    Ok(chunk) if errors.is_empty() => Ok(chunk),
                    Ok(chunk) => Err(LensError::with_partial_chunk(chunk, errors)),
                    Err(err) => {
                        errors.push(err.into());
                        Err(LensError::new(None, errors))
                    }
                }
            })
        }
    };

    // --- Produce derived chunks ---

    let derived_chunks = derive_work.into_iter().map(|work| match work.rows {
        Rows::OneToMany => apply_one_to_many(&work),
        Rows::OneToOne => apply_one_to_one(&work),
    });

    // --- Chain all results ---

    prefix.into_iter().chain(derived_chunks)
}
