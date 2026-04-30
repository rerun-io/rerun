//! Execution planning for lenses.
//!
//! Categorizes lenses into work buckets (mutate, merge-into-prefix,
//! derive) and detects output collisions.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use nohash_hasher::IntMap;
use re_chunk::{Chunk, ComponentIdentifier, EntityPath, TimeColumn, TimelineName};
use re_sdk_types::SerializedComponentColumn;

use crate::Selector;
use re_log_types::ResolvedEntityPathFilter;

use crate::ast::{ComponentOutput, Lenses, OutputMode, Rows, TimeOutput};
use crate::error::LensRuntimeError;
use crate::selector::DynExpr;

pub type ChunkTimelines = IntMap<TimelineName, TimeColumn>;

/// Work item for a mutate lens.
pub struct MutateWork<'a> {
    pub selector: &'a Selector<DynExpr>,
    pub keep_row_ids: bool,
}

/// Work item for a same-entity derive that merges into the prefix chunk.
pub struct MergeWork<'a> {
    pub components: &'a [ComponentOutput],
    pub input: &'a SerializedComponentColumn,
}

/// Work item for a derive lens that produces a separate output chunk.
pub struct DeriveWork<'a> {
    pub rows: Rows,
    pub input: &'a SerializedComponentColumn,
    pub target_entity: &'a EntityPath,
    pub components: &'a [ComponentOutput],
    pub timelines: &'a [TimeOutput],
    pub original_timelines: &'a ChunkTimelines,
}

/// The execution plan produced by categorizing lenses.
pub struct Plan<'a> {
    pub mutate_work: BTreeMap<ComponentIdentifier, MutateWork<'a>>,
    pub merge_work: Vec<MergeWork<'a>>,
    pub derive_work: Vec<DeriveWork<'a>>,

    /// Original columns to include in the prefix chunk.
    pub forward_columns: BTreeSet<ComponentIdentifier>,

    /// Errors detected during planning (e.g. mutate collisions).
    pub errors: Vec<LensRuntimeError>,
}

/// Returns lenses whose entity path filter matches the given path.
fn matching_lenses<'a, L>(
    lenses: &'a [(ResolvedEntityPathFilter, L)],
    entity_path: &'a EntityPath,
) -> impl Iterator<Item = &'a L> {
    lenses
        .iter()
        .filter(move |(filter, _)| filter.matches(entity_path))
        .map(|(_, lens)| lens)
}

/// Returns `true` if any output component collides with an already-claimed identifier.
fn has_output_collision(
    entity_path: &EntityPath,
    outputs: &[ComponentOutput],
    claimed: &mut BTreeSet<ComponentIdentifier>,
    errors: &mut Vec<LensRuntimeError>,
) -> bool {
    let mut collision = false;
    for comp_out in outputs {
        let output_id = comp_out.component_descr.component;
        if !claimed.insert(output_id) {
            errors.push(LensRuntimeError::DeriveCollision {
                entity_path: entity_path.clone(),
                component: output_id,
            });
            collision = true;
        }
    }
    collision
}

/// Builds the execution plan from pre-categorized lenses.
///
/// Filters lenses by entity path, resolves input columns against the chunk,
/// detects output collisions, and determines which original columns to forward.
pub fn plan<'a>(lenses: &'a Lenses, chunk: &'a Chunk) -> Plan<'a> {
    let entity_path = chunk.entity_path();

    // --- Mutates ---
    let mut mutate_work = BTreeMap::<ComponentIdentifier, MutateWork<'_>>::new();
    let mut errors = Vec::<LensRuntimeError>::new();
    {
        for lens in matching_lenses(&lenses.mutates, entity_path)
            .filter(|lens| chunk.components().contains_component(lens.input))
        {
            if let Entry::Vacant(entry) = mutate_work.entry(lens.input) {
                entry.insert(MutateWork {
                    selector: &lens.selector,
                    keep_row_ids: lens.keep_row_ids,
                });
            } else {
                errors.push(LensRuntimeError::MutateCollision {
                    entity_path: entity_path.clone(),
                    component: lens.input,
                });
            }
        }
    }

    // --- Same-entity derives ---
    let mut merge_work = Vec::<MergeWork<'_>>::new();
    let mut derive_work = Vec::<DeriveWork<'_>>::new();
    let track_consumed = lenses.mode == OutputMode::ForwardUnmatched;
    let mut consumed = BTreeSet::<ComponentIdentifier>::new();
    {
        let mut claimed = BTreeSet::<ComponentIdentifier>::new();
        for derive in matching_lenses(&lenses.same_entity_derives, entity_path) {
            let Some(input) = chunk.components().get(derive.input) else {
                continue;
            };

            if track_consumed && !mutate_work.contains_key(&derive.input) {
                consumed.insert(derive.input);
            }

            if has_output_collision(
                entity_path,
                &derive.output_components,
                &mut claimed,
                &mut errors,
            ) {
                continue;
            }

            if derive.is_merge_candidate() {
                merge_work.push(MergeWork {
                    components: &derive.output_components,
                    input,
                });
            } else {
                derive_work.push(DeriveWork {
                    rows: derive.rows,
                    input,
                    target_entity: entity_path,
                    components: &derive.output_components,
                    timelines: &derive.output_timelines,
                    original_timelines: chunk.timelines(),
                });
            }
        }
    }

    // --- Separate-entity derives ---
    {
        let mut claimed = BTreeMap::<&EntityPath, BTreeSet<ComponentIdentifier>>::new();
        for derive in matching_lenses(&lenses.separate_entity_derives, entity_path) {
            let Some(input) = chunk.components().get(derive.input) else {
                continue;
            };

            if track_consumed && !mutate_work.contains_key(&derive.input) {
                consumed.insert(derive.input);
            }

            if !has_output_collision(
                &derive.target_entity,
                &derive.output_components,
                claimed.entry(&derive.target_entity).or_default(),
                &mut errors,
            ) {
                derive_work.push(DeriveWork {
                    rows: derive.rows,
                    input,
                    target_entity: &derive.target_entity,
                    components: &derive.output_components,
                    timelines: &derive.output_timelines,
                    original_timelines: chunk.timelines(),
                });
            }
        }
    }

    // --- Resolve which original columns to forward ---
    let forward_columns = match lenses.mode {
        OutputMode::ForwardAll => chunk.components().keys().copied().collect(),
        OutputMode::ForwardUnmatched => chunk
            .components()
            .keys()
            .copied()
            .filter(|id| !consumed.contains(id))
            .collect(),
        OutputMode::DropUnmatched => mutate_work.keys().copied().collect(),
    };

    Plan {
        mutate_work,
        merge_work,
        derive_work,
        forward_columns,
        errors,
    }
}
