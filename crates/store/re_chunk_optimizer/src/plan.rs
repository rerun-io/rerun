//! The planner: a pure function from a [`ChunkIndexView`] and [`OptimizationSettings`] to a flat
//! list of plan units.
//!
//! The plan's atom is a [`ChunkSlice`]: a column subset of one chunk. Every unit names slices, so
//! a column split is one more unit over the same chunks and the plan stays a flat list.
//!
//! # Completeness
//!
//! The plan must route every non-null cell of every chunk to exactly one unit. For each input
//! chunk, there should be either:
//!
//! - one slice, [`ColumnSelection::All`]; or
//! - distinct [`ColumnSelection::Only`] columns plus one [`ColumnSelection::Except`] naming exactly
//!   those columns; or
//! - distinct [`ColumnSelection::Only`] columns, at least one, that are exactly the chunk's
//!   [`ChunkMeta::components`].

use std::collections::{BTreeMap, BTreeSet};

use re_chunk::{ComponentIdentifier, ComponentType};
use re_log_types::{EntityPath, EntityPathFilter, ResolvedEntityPathFilter, TimelineName};

use crate::settings::{
    ColumnSelector, MergeSplitOverride, MergeSplitSettings, OptimizationSettings, OwnChunkRule,
};
use crate::view::{ChunkIdx, ChunkIndexView, ChunkMeta, TimelineSetGroup};

/// A column subset of one chunk of the index.
// Note: column only for now, will eventually extend to row windows as well.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkSlice {
    pub chunk: ChunkIdx,
    pub columns: ColumnSelection,
}

impl ChunkSlice {
    pub fn all(chunk: ChunkIdx) -> Self {
        Self {
            chunk,
            columns: ColumnSelection::All,
        }
    }

    #[inline]
    pub fn only(chunk: ChunkIdx, columns: impl IntoIterator<Item = ComponentIdentifier>) -> Self {
        Self {
            chunk,
            columns: ColumnSelection::Only(columns.into_iter().collect()),
        }
    }

    #[inline]
    pub fn except(chunk: ChunkIdx, columns: impl IntoIterator<Item = ComponentIdentifier>) -> Self {
        Self {
            chunk,
            columns: ColumnSelection::Except(columns.into_iter().collect()),
        }
    }
}

/// The component columns a [`ChunkSlice`] keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnSelection {
    All,

    /// Exactly these columns. Never empty.
    Only(BTreeSet<ComponentIdentifier>),

    /// Every column not named here. Never empty: [`plan_covers_view`] rejects an empty set.
    Except(BTreeSet<ComponentIdentifier>),
}

/// One piece of the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanUnit {
    /// Emit the slice as-is.
    Passthrough(ChunkSlice),

    /// Load the slices in order, merging/splitting them on the way per the target settings.
    ///
    /// The merge-and-split run is executed in the order provided. The result (including chunk
    /// count) is order dependant.
    ///
    /// # Implementation note
    ///
    /// This is deliberately "vague" (as opposed to, say, a rigid `Merge` unit) because planning
    /// merges towards a target size is, in general, impossible, see RR-5536. The TL;DR is that, to
    /// plan towards a final chunk size, the per-chunk framing overhead in input chunks must be
    /// known. The index currently does not provide enough information for that.
    MergeSplitRun {
        inputs: Vec<ChunkSlice>,
        target: MergeSplitSettings,
    },
}

/// The own columns of one entity, each with the target its run merges toward, or `None` when its
/// slices pass through.
type OwnColumns = BTreeMap<ComponentIdentifier, Option<MergeSplitSettings>>;

/// Build a plan.
pub fn plan(view: &ChunkIndexView, settings: &OptimizationSettings) -> Vec<PlanUnit> {
    let mut units = Vec::new();
    let mut claimed = vec![false; view.num_chunks()];

    // Resolved for every entity up front: the static and orphan loops below are global.
    let own_columns_by_entity = resolve_own_columns(view, settings);
    let no_own_columns = OwnColumns::new();
    let own_columns_of = |entity_path: &EntityPath| {
        own_columns_by_entity
            .get(entity_path)
            .unwrap_or(&no_own_columns)
    };

    // Static chunks pass through, in chunk-index order, split on their own columns.
    for (idx, meta) in view.chunks() {
        if meta.is_static {
            claimed[idx.as_usize()] = true;
            units.extend(
                slices_of_chunk(idx, meta, own_columns_of(&meta.entity_path))
                    .into_iter()
                    .map(PlanUnit::Passthrough),
            );
        }
    }

    // Temporal chunks merge per entity and per exact timeline set — the same grouping the merge
    // gate `Chunk::concatenable` enforces at merge time. Within a group, every own column gets a
    // run of its own over the chunks that carry it, then the rest of every chunk forms one run.
    for (entity_path, entity) in &view.entities {
        let own = own_columns_of(entity_path);
        for group in &entity.timeline_sets {
            let order = sweep_order(view, group, settings.target_timeline.as_ref());
            for &idx in &order {
                claimed[idx.as_usize()] = true;
            }

            let mut own_runs: BTreeMap<ComponentIdentifier, Vec<ChunkSlice>> = BTreeMap::new();
            let mut rest = Vec::new();
            for &idx in &order {
                let meta = view.chunk(idx);
                let present = present_own_columns(meta, own);
                for &column in &present {
                    own_runs
                        .entry(column)
                        .or_default()
                        .push(ChunkSlice::only(idx, [column]));
                }
                rest.extend(rest_slice(idx, meta, present));
            }

            for (column, inputs) in own_runs {
                emit(&mut units, inputs, own[&column]);
            }
            if !rest.is_empty() {
                emit(&mut units, rest, settings.merge_split);
            }
        }
    }

    // A chunk in neither bucket — non-static, yet with a null time range on every
    // (index, component) pair, so the temporal map never saw it — must still reach the output.
    for (idx, meta) in view.chunks() {
        if !claimed[idx.as_usize()] {
            re_log::warn_once!(
                "Chunk is neither static nor on any timeline; passing it through unoptimized. \
                 Chunk id: {}\nEntity: {}",
                meta.chunk_id,
                meta.entity_path,
            );
            units.extend(
                slices_of_chunk(idx, meta, own_columns_of(&meta.entity_path))
                    .into_iter()
                    .map(PlanUnit::Passthrough),
            );
        }
    }

    re_log::debug_assert!(
        plan_covers_view(view, &units),
        "every column of every chunk must land in exactly one unit"
    );

    units
}

fn emit(units: &mut Vec<PlanUnit>, inputs: Vec<ChunkSlice>, target: Option<MergeSplitSettings>) {
    match target {
        Some(target) => units.push(PlanUnit::MergeSplitRun { inputs, target }),
        None => units.extend(inputs.into_iter().map(PlanUnit::Passthrough)),
    }
}

fn slices_of_chunk(idx: ChunkIdx, meta: &ChunkMeta, own: &OwnColumns) -> Vec<ChunkSlice> {
    let present = present_own_columns(meta, own);
    let mut slices: Vec<ChunkSlice> = present
        .iter()
        .map(|&column| ChunkSlice::only(idx, [column]))
        .collect();
    slices.extend(rest_slice(idx, meta, present));
    slices
}

fn present_own_columns(meta: &ChunkMeta, own: &OwnColumns) -> BTreeSet<ComponentIdentifier> {
    own.keys()
        .filter(|column| meta.components.contains_key(column))
        .copied()
        .collect()
}

/// `None` when every column of the chunk is an own column: a dedicated chunk has nothing left to
/// load.
fn rest_slice(
    idx: ChunkIdx,
    meta: &ChunkMeta,
    present: BTreeSet<ComponentIdentifier>,
) -> Option<ChunkSlice> {
    if present.is_empty() {
        Some(ChunkSlice::all(idx))
    } else if present.len() == meta.components.len() {
        None
    } else {
        Some(ChunkSlice::except(idx, present))
    }
}

/// Resolve [`OptimizationSettings::own_chunk`] per entity.
///
/// [`ColumnSelector::Type`] matches when any chunk of the entity carries the column under that
/// type; the match then holds for the identifier, so every chunk carrying it is sliced, whatever
/// type that chunk records.
fn resolve_own_columns(
    view: &ChunkIndexView,
    settings: &OptimizationSettings,
) -> BTreeMap<EntityPath, OwnColumns> {
    if settings.own_chunk.is_empty() {
        return BTreeMap::new();
    }

    // Per entity and column: the types the entity's chunks carry for the column.
    let mut types: BTreeMap<&EntityPath, BTreeMap<ComponentIdentifier, BTreeSet<ComponentType>>> =
        BTreeMap::new();
    for (_, meta) in view.chunks() {
        let entity_types = types.entry(&meta.entity_path).or_default();
        for (&column, component_type) in &meta.components {
            let column_types = entity_types.entry(column).or_default();
            column_types.extend(component_type);
        }
    }

    let rules: Vec<(Option<ResolvedEntityPathFilter>, &OwnChunkRule)> = settings
        .own_chunk
        .iter()
        .map(|rule| {
            (
                rule.entity_filter
                    .clone()
                    .map(EntityPathFilter::resolve_without_substitutions),
                rule,
            )
        })
        .collect();

    types
        .into_iter()
        .map(|(entity_path, columns)| {
            let own: OwnColumns = columns
                .into_iter()
                .filter_map(|(column, column_types)| {
                    let (_, rule) = rules.iter().find(|(filter, rule)| {
                        filter
                            .as_ref()
                            .is_none_or(|filter| filter.matches(entity_path))
                            && match rule.column {
                                ColumnSelector::Type(t) => column_types.contains(&t),
                                ColumnSelector::Column(c) => c == column,
                            }
                    })?;
                    let target = match rule.merge_split {
                        MergeSplitOverride::Inherit => settings.merge_split,
                        MergeSplitOverride::Passthrough => None,
                        MergeSplitOverride::MergeSplit(target) => Some(target),
                    };
                    Some((column, target))
                })
                .collect();
            (entity_path.clone(), own)
        })
        .collect()
}

/// The order in which a group's chunks are swept into its merge/split run.
///
/// Currently:
/// - use time-based ordering if a target timeline is specified
/// - retain input-file order otherwise (based on chunk's byte offset)
// TODO(ab): this needs more efforts: there actually exists index-based ordering decisions that can
// predictably improve the merge/split result.
fn sweep_order(
    view: &ChunkIndexView,
    group: &TimelineSetGroup,
    target_timeline: Option<&TimelineName>,
) -> Vec<ChunkIdx> {
    if let Some(target) = target_timeline
        && let Some(timeline) = group.timelines.iter().find(|t| t.name() == target)
    {
        // Note: chunks within `per_timeline` are already sorted by range start
        return group.per_timeline[timeline]
            .iter()
            .map(|span| span.chunk)
            .collect();
    }

    let mut idxs: Vec<ChunkIdx> = group
        .per_timeline
        .values()
        .next()
        .into_iter()
        .flatten()
        .map(|span| span.chunk)
        .collect();
    idxs.sort_by_key(|&idx| (view.chunk(idx).rrd_byte_offset, idx));
    idxs
}

/// Check the plan invariant: the slices of every chunk of the view, across all units, form one of
/// the three partitions the module doc lists, and no run is empty.
pub fn plan_covers_view(view: &ChunkIndexView, units: &[PlanUnit]) -> bool {
    let mut slices: Vec<Vec<&ColumnSelection>> = vec![Vec::new(); view.num_chunks()];

    for unit in units {
        match unit {
            PlanUnit::Passthrough(slice) => {
                slices[slice.chunk.as_usize()].push(&slice.columns);
            }

            PlanUnit::MergeSplitRun { inputs, target: _ } => {
                if inputs.is_empty() {
                    return false;
                }
                for slice in inputs {
                    slices[slice.chunk.as_usize()].push(&slice.columns);
                }
            }
        }
    }

    view.chunks()
        .all(|(idx, meta)| slices_correctly_partition_chunk(&slices[idx.as_usize()], meta))
}

/// Sanity check that the input slice fully cover the input chunk without overlap.
fn slices_correctly_partition_chunk(slices: &[&ColumnSelection], chunk_meta: &ChunkMeta) -> bool {
    // only acceptable slices containing `All`
    if let [ColumnSelection::All] = slices {
        return true;
    }

    let mut onlys: BTreeSet<ComponentIdentifier> = BTreeSet::new();
    let mut except: Option<&BTreeSet<ComponentIdentifier>> = None;
    for selection in slices {
        match selection {
            ColumnSelection::All => return false,

            ColumnSelection::Only(set) => {
                if set.is_empty() || !onlys.is_disjoint(set) {
                    return false;
                }
                onlys.extend(set.iter().copied());
            }

            ColumnSelection::Except(set) => {
                // zero or one `Except` is allowed
                if set.is_empty() || except.replace(set).is_some() {
                    return false;
                }
            }
        }
    }

    // columns listed in the one `Except` (if any) must be covered with `Only`s
    match except {
        Some(set) => *set == onlys,
        None => !onlys.is_empty() && chunk_meta.components.keys().eq(onlys.iter()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::num::NonZeroU64;

    use re_chunk::{Chunk, ChunkId, ComponentIdentifier, RowId};
    use re_log_encoding::RawRrdManifest;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{EntityPathFilter, StoreId, StoreKind, Timeline};
    use re_types_core::{Component as _, ComponentBatch as _, ComponentDescriptor};

    use super::{ChunkSlice, PlanUnit, plan, plan_covers_view};
    use crate::view::{ChunkIdx, ChunkIndexView};
    use crate::{
        ColumnSelector, MergeSplitOverride, MergeSplitSettings, OptimizationSettings, OwnChunkRule,
    };

    /// A temporal chunk on the `frame` timeline: one row per time, `points_per_row` points each.
    fn temporal_chunk(id: u128, entity: &str, times: &[i64], points_per_row: u32) -> Chunk {
        let frame = Timeline::new_sequence("frame");
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for (i, &time) in times.iter().enumerate() {
            builder = builder.with_serialized_batches(
                RowId::from_u128((id << 32) + i as u128 + 1),
                [(frame, time)],
                [MyPoint::from_iter(0..points_per_row)
                    .try_serialized(MyPoints::descriptor_points())
                    .unwrap()],
            );
        }
        builder.build().unwrap()
    }

    /// A temporal chunk with a colors column only.
    fn temporal_color_chunk(id: u128, entity: &str, times: &[i64], colors_per_row: u32) -> Chunk {
        let frame = Timeline::new_sequence("frame");
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for (i, &time) in times.iter().enumerate() {
            builder = builder.with_serialized_batches(
                RowId::from_u128((id << 32) + i as u128 + 1),
                [(frame, time)],
                [MyColor::from_iter(0..colors_per_row)
                    .try_serialized(MyPoints::descriptor_colors())
                    .unwrap()],
            );
        }
        builder.build().unwrap()
    }

    /// A temporal chunk with both a points and a colors column on every row.
    fn temporal_mixed_chunk(
        id: u128,
        entity: &str,
        times: &[i64],
        points_per_row: u32,
        colors_per_row: u32,
    ) -> Chunk {
        let frame = Timeline::new_sequence("frame");
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for (i, &time) in times.iter().enumerate() {
            builder = builder.with_serialized_batches(
                RowId::from_u128((id << 32) + i as u128 + 1),
                [(frame, time)],
                [
                    MyPoint::from_iter(0..points_per_row)
                        .try_serialized(MyPoints::descriptor_points())
                        .unwrap(),
                    MyColor::from_iter(0..colors_per_row)
                        .try_serialized(MyPoints::descriptor_colors())
                        .unwrap(),
                ],
            );
        }
        builder.build().unwrap()
    }

    fn untyped_descriptor() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype: None,
            component: "custom".into(),
            component_type: None,
        }
    }

    /// A temporal chunk with a points column and a column whose descriptor carries no type.
    fn temporal_untyped_chunk(id: u128, entity: &str, times: &[i64]) -> Chunk {
        let frame = Timeline::new_sequence("frame");
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for (i, &time) in times.iter().enumerate() {
            builder = builder.with_serialized_batches(
                RowId::from_u128((id << 32) + i as u128 + 1),
                [(frame, time)],
                [
                    MyPoint::from_iter(0..2)
                        .try_serialized(MyPoints::descriptor_points())
                        .unwrap(),
                    MyPoint::from_iter(0..2)
                        .try_serialized(untyped_descriptor())
                        .unwrap(),
                ],
            );
        }
        builder.build().unwrap()
    }

    fn static_chunk(id: u128, entity: &str) -> Chunk {
        Chunk::builder_with_id(ChunkId::from_u128(id), entity)
            .with_serialized_batches(
                RowId::from_u128(id << 32),
                re_log_types::TimePoint::default(),
                [MyPoint::from_iter(0..1)
                    .try_serialized(MyPoints::descriptor_points())
                    .unwrap()],
            )
            .build()
            .unwrap()
    }

    fn static_mixed_chunk(id: u128, entity: &str) -> Chunk {
        Chunk::builder_with_id(ChunkId::from_u128(id), entity)
            .with_serialized_batches(
                RowId::from_u128(id << 32),
                re_log_types::TimePoint::default(),
                [
                    MyPoint::from_iter(0..1)
                        .try_serialized(MyPoints::descriptor_points())
                        .unwrap(),
                    MyColor::from_iter(0..1)
                        .try_serialized(MyPoints::descriptor_colors())
                        .unwrap(),
                ],
            )
            .build()
            .unwrap()
    }

    fn view_of(chunks: &[Chunk]) -> ChunkIndexView {
        let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
        let chunk_index =
            RawRrdManifest::build_in_memory_from_chunks(store_id, chunks.iter()).unwrap();
        ChunkIndexView::try_from_raw(&chunk_index).unwrap()
    }

    fn idx_of(view: &ChunkIndexView, id: u128) -> ChunkIdx {
        let id = ChunkId::from_u128(id);
        view.chunks()
            .find(|(_, meta)| meta.chunk_id == id)
            .map(|(idx, _)| idx)
            .unwrap()
    }

    fn colors() -> ComponentIdentifier {
        MyPoints::descriptor_colors().component
    }

    fn color_set() -> BTreeSet<ComponentIdentifier> {
        std::iter::once(colors()).collect()
    }

    fn merge_split_settings(max_bytes: u64, max_rows: u64) -> MergeSplitSettings {
        MergeSplitSettings {
            max_bytes: NonZeroU64::new(max_bytes).unwrap(),
            max_rows: NonZeroU64::new(max_rows),
            max_rows_if_unsorted: None,
        }
    }

    fn settings(max_bytes: u64, max_rows: u64) -> OptimizationSettings {
        OptimizationSettings {
            merge_split: Some(merge_split_settings(max_bytes, max_rows)),
            target_timeline: None,
            own_chunk: Vec::new(),
        }
    }

    fn with_rules(
        mut settings: OptimizationSettings,
        rules: Vec<OwnChunkRule>,
    ) -> OptimizationSettings {
        settings.own_chunk = rules;
        settings
    }

    fn passthrough_all(view: &ChunkIndexView, id: u128) -> PlanUnit {
        PlanUnit::Passthrough(ChunkSlice::all(idx_of(view, id)))
    }

    fn merge_split_run(
        view: &ChunkIndexView,
        ids: &[u128],
        settings: &OptimizationSettings,
    ) -> PlanUnit {
        PlanUnit::MergeSplitRun {
            inputs: ids
                .iter()
                .map(|&id| ChunkSlice::all(idx_of(view, id)))
                .collect(),
            target: settings.merge_split.unwrap(),
        }
    }

    fn only_run(
        view: &ChunkIndexView,
        ids: &[u128],
        column: ComponentIdentifier,
        target: MergeSplitSettings,
    ) -> PlanUnit {
        PlanUnit::MergeSplitRun {
            inputs: ids
                .iter()
                .map(|&id| ChunkSlice::only(idx_of(view, id), [column]))
                .collect(),
            target,
        }
    }

    /// A group's chunks form one merge/split run in sweep order, whatever the byte target —
    /// output boundaries and splits belong to the executor.
    #[test]
    fn group_forms_one_run() {
        let chunks: Vec<Chunk> = (0..7)
            .map(|i| temporal_chunk(i + 1, "entity", &[i as i64 * 10, i as i64 * 10 + 1], 64))
            .collect();
        let view = view_of(&chunks);

        let settings = settings(1, 0); // even a tiny target: the plan shape does not change
        let outputs = plan(&view, &settings);
        assert!(plan_covers_view(&view, &outputs));

        let expected = vec![merge_split_run(&view, &[1, 2, 3, 4, 5, 6, 7], &settings)];
        assert_eq!(outputs, expected);
    }

    /// Chunks never share a run across entities or across timeline sets; a singleton group is a
    /// one-input run.
    #[test]
    fn one_run_per_group() {
        let frame = Timeline::new_sequence("frame");
        let other = Timeline::new_sequence("other");

        // Two chunks on `frame` alone, one chunk on `frame` + `other`, one chunk on another
        // entity: three groups, no run across them.
        let two_timelines = {
            let mut builder = Chunk::builder_with_id(ChunkId::from_u128(3), "entity");
            for i in 0..2_i64 {
                builder = builder.with_serialized_batches(
                    RowId::from_u128((3 << 32) + i as u128 + 1),
                    [(frame, i), (other, i)],
                    [MyPoint::from_iter(0..64)
                        .try_serialized(MyPoints::descriptor_points())
                        .unwrap()],
                );
            }
            builder.build().unwrap()
        };
        let chunks = vec![
            temporal_chunk(1, "entity", &[0, 1], 64),
            temporal_chunk(2, "entity", &[10, 11], 64),
            two_timelines,
            temporal_chunk(4, "other_entity", &[0, 1], 64),
        ];
        let view = view_of(&chunks);

        let settings = settings(u64::MAX / 2, 0);
        let outputs = plan(&view, &settings);
        assert!(plan_covers_view(&view, &outputs));

        let expected = vec![
            merge_split_run(&view, &[1, 2], &settings),
            merge_split_run(&view, &[3], &settings),
            merge_split_run(&view, &[4], &settings),
        ];
        assert_eq!(outputs, expected);
    }

    /// Statics pass through untouched; `merge_split: None` disables the optimization — every
    /// temporal chunk passes through, and no run exists.
    #[test]
    fn passthrough_rules() {
        let chunks = vec![
            static_chunk(1, "static_entity"),
            temporal_chunk(2, "entity", &[0, 1], 64),
            temporal_chunk(3, "entity", &[10, 11], 64),
        ];
        let view = view_of(&chunks);

        // Tiny byte target: statics still pass through whole; temporal chunks form their run.
        let outputs = plan(&view, &settings(1, 0));
        assert!(plan_covers_view(&view, &outputs));
        assert_eq!(outputs[0], passthrough_all(&view, 1));
        assert!(matches!(&outputs[1], PlanUnit::MergeSplitRun { .. }));

        // `merge_split: None`: every chunk stands alone, no run.
        let disabled = OptimizationSettings {
            merge_split: None,
            target_timeline: None,
            own_chunk: Vec::new(),
        };
        let outputs = plan(&view, &disabled);
        assert!(plan_covers_view(&view, &outputs));
        assert_eq!(
            outputs,
            vec![
                passthrough_all(&view, 1),
                passthrough_all(&view, 2),
                passthrough_all(&view, 3),
            ]
        );
    }

    /// With `target_timeline`, the run's inputs follow time order where file order disagrees; a
    /// group lacking the timeline, or an unknown name, falls back to file order.
    #[test]
    fn sweep_order() {
        // Written (file) order: times 0, 20, 10, 30.
        let chunks = vec![
            temporal_chunk(1, "entity", &[0, 1], 64),
            temporal_chunk(2, "entity", &[20, 21], 64),
            temporal_chunk(3, "entity", &[10, 11], 64),
            temporal_chunk(4, "entity", &[30, 31], 64),
        ];
        let view = view_of(&chunks);

        let with_target = |name: &str| OptimizationSettings {
            merge_split: settings(1024 * 1024, 0).merge_split,
            target_timeline: Some(re_log_types::TimelineName::try_new(name).unwrap()),
            own_chunk: Vec::new(),
        };

        // Time order sweeps 0, 10, 20, 30.
        let outputs = plan(&view, &with_target("frame"));
        assert!(plan_covers_view(&view, &outputs));
        assert_eq!(
            outputs,
            vec![merge_split_run(&view, &[1, 3, 2, 4], &with_target("frame"))]
        );

        // File order sweeps 0, 20, 10, 30 — both with no target and with an unknown one.
        let file_order = vec![merge_split_run(
            &view,
            &[1, 2, 3, 4],
            &settings(1024 * 1024, 0),
        )];
        assert_eq!(plan(&view, &settings(1024 * 1024, 0)), file_order);
        assert_eq!(
            plan(&view, &with_target("no_such_timeline")),
            vec![merge_split_run(
                &view,
                &[1, 2, 3, 4],
                &with_target("no_such_timeline")
            )]
        );
    }

    /// The chunk index of `chunks` with the second chunk's per-(index, component) time ranges
    /// nulled out, so that the temporal map never sees it.
    fn view_with_orphan(chunks: &[Chunk]) -> ChunkIndexView {
        use arrow::array::BooleanArray;

        let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
        let mut raw = RawRrdManifest::build_in_memory_from_chunks(store_id, chunks.iter()).unwrap();

        // Rows follow append order, so row 1 is the second chunk.
        let mut mask = vec![false; chunks.len()];
        mask[1] = true;
        let orphan_mask = BooleanArray::from(mask);
        let schema = raw.data.schema();
        let columns = std::iter::zip(schema.fields(), raw.data.columns())
            .map(|(field, column)| {
                // The same identification `calc_temporal_map` uses for the per-(index, component)
                // time-range pair columns.
                let is_pair_range = RawRrdManifest::is_index(field)
                    && RawRrdManifest::is_index_per_component(field)
                    && (RawRrdManifest::is_index_start(field)
                        || RawRrdManifest::is_index_end(field));
                if is_pair_range {
                    arrow::compute::nullif(column, &orphan_mask).unwrap()
                } else {
                    column.clone()
                }
            })
            .collect();
        let row_count = raw.data.num_rows();
        raw.data = arrow::array::RecordBatch::try_new_with_options(
            schema,
            columns,
            &arrow::array::RecordBatchOptions::new().with_row_count(Some(row_count)),
        )
        .unwrap();

        ChunkIndexView::try_from_raw(&raw).unwrap()
    }

    /// A non-static chunk whose time range is null on every (index, component) pair is invisible
    /// to the temporal map, so it lands in no group — the plan must still pass it through instead
    /// of silently dropping its rows.
    #[test]
    fn orphan_chunk_passes_through() {
        let chunks = [
            temporal_chunk(1, "entity", &[0, 1], 64),
            temporal_chunk(2, "entity", &[10, 11], 64),
        ];
        let view = view_with_orphan(&chunks);
        let settings = settings(1024 * 1024, 0);
        let units = plan(&view, &settings);

        assert!(plan_covers_view(&view, &units));
        assert_eq!(
            units,
            vec![
                merge_split_run(&view, &[1], &settings),
                passthrough_all(&view, 2),
            ]
        );
    }

    /// Own-column runs hold `Only` slices of every chunk carrying the column, dedicated chunks
    /// included; the rest run holds `Except` slices of the mixed chunks, `All` of the chunks
    /// without the column, and nothing of the dedicated chunks. A group of dedicated chunks only
    /// has no rest run.
    #[test]
    fn slices_follow_the_index() {
        let other = Timeline::new_sequence("other");
        let dedicated_on_other = |id: u128| {
            Chunk::builder_with_id(ChunkId::from_u128(id), "entity")
                .with_serialized_batches(
                    RowId::from_u128(id << 32),
                    [(other, 0_i64)],
                    [MyColor::from_iter(0..4)
                        .try_serialized(MyPoints::descriptor_colors())
                        .unwrap()],
                )
                .build()
                .unwrap()
        };
        let chunks = vec![
            temporal_mixed_chunk(1, "entity", &[0, 1], 4, 4),
            temporal_chunk(2, "entity", &[10, 11], 4),
            temporal_color_chunk(3, "entity", &[20, 21], 4),
            temporal_mixed_chunk(4, "entity", &[30, 31], 4, 4),
            // A second group, on another timeline, of dedicated colors chunks only.
            dedicated_on_other(5),
            dedicated_on_other(6),
        ];
        let view = view_of(&chunks);

        let settings = with_rules(
            settings(1024 * 1024, 0),
            vec![OwnChunkRule::new(ColumnSelector::Type(MyColor::name()))],
        );
        let target = settings.merge_split.unwrap();
        let units = plan(&view, &settings);
        assert!(plan_covers_view(&view, &units));

        let except_colors =
            |id: u128| ChunkSlice::except(idx_of(&view, id), BTreeSet::from([colors()]));
        assert_eq!(
            units,
            vec![
                only_run(&view, &[1, 3, 4], colors(), target),
                PlanUnit::MergeSplitRun {
                    inputs: vec![
                        except_colors(1),
                        ChunkSlice::all(idx_of(&view, 2)),
                        except_colors(4),
                    ],
                    target,
                },
                only_run(&view, &[5, 6], colors(), target),
            ]
        );
    }

    /// Rules are tried in order; the selector kinds match what they say; the entity filter
    /// applies; every override resolves as documented, with and without a global target.
    #[test]
    fn rule_resolution() {
        let chunks = vec![
            temporal_mixed_chunk(1, "entity", &[0, 1], 4, 4),
            temporal_mixed_chunk(2, "other", &[0, 1], 4, 4),
            temporal_mixed_chunk(3, "__properties/x", &[0, 1], 4, 4),
            temporal_untyped_chunk(4, "untyped", &[0, 1]),
        ];
        let view = view_of(&chunks);
        let global = settings(1024 * 1024, 0);
        let target = global.merge_split.unwrap();
        let custom = untyped_descriptor().component;

        let type_rule = OwnChunkRule::new(ColumnSelector::Type(MyColor::name()));
        let column_passthrough = OwnChunkRule {
            merge_split: MergeSplitOverride::Passthrough,
            ..OwnChunkRule::new(ColumnSelector::Column(colors()))
        };

        // The units holding a colors slice of chunk 1 (entity `entity`) under the given rules.
        let colors_units = |settings: &OptimizationSettings, rules: Vec<OwnChunkRule>| {
            let units = plan(&view, &with_rules(settings.clone(), rules));
            assert!(plan_covers_view(&view, &units));
            let is_colors_of_1 = |slice: &ChunkSlice| {
                slice.chunk == idx_of(&view, 1)
                    && slice.columns == super::ColumnSelection::Only(color_set())
            };
            units
                .into_iter()
                .filter(|unit| match unit {
                    PlanUnit::Passthrough(slice) => is_colors_of_1(slice),
                    PlanUnit::MergeSplitRun { inputs, .. } => inputs.iter().any(is_colors_of_1),
                })
                .collect::<Vec<_>>()
        };

        // Rules are tried in order: a run with the type rule first, a passthrough with the column
        // rule first.
        assert_eq!(
            colors_units(&global, vec![type_rule.clone(), column_passthrough.clone()]),
            vec![only_run(&view, &[1], colors(), target)]
        );
        assert_eq!(
            colors_units(&global, vec![column_passthrough.clone(), type_rule.clone()]),
            vec![PlanUnit::Passthrough(ChunkSlice::only(
                idx_of(&view, 1),
                color_set()
            ))]
        );

        // An untyped column matches `Column`, never `Type`.
        let units_of = |rules: Vec<OwnChunkRule>| {
            let units = plan(&view, &with_rules(global.clone(), rules));
            assert!(plan_covers_view(&view, &units));
            units
        };
        let untyped_units = |units: &[PlanUnit]| {
            units
                .iter()
                .filter(|unit| match unit {
                    PlanUnit::Passthrough(slice) => slice.chunk == idx_of(&view, 4),
                    PlanUnit::MergeSplitRun { inputs, .. } => {
                        inputs.iter().any(|s| s.chunk == idx_of(&view, 4))
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        let by_type = units_of(vec![OwnChunkRule::new(ColumnSelector::Type(
            MyPoint::name(),
        ))]);
        assert_eq!(
            untyped_units(&by_type),
            vec![
                only_run(&view, &[4], MyPoints::descriptor_points().component, target),
                PlanUnit::MergeSplitRun {
                    inputs: vec![ChunkSlice::except(
                        idx_of(&view, 4),
                        [MyPoints::descriptor_points().component]
                    )],
                    target,
                },
            ]
        );
        let by_column = units_of(vec![OwnChunkRule::new(ColumnSelector::Column(custom))]);
        assert_eq!(
            untyped_units(&by_column),
            vec![
                only_run(&view, &[4], custom, target),
                PlanUnit::MergeSplitRun {
                    inputs: vec![ChunkSlice::except(idx_of(&view, 4), [custom])],
                    target,
                },
            ]
        );

        // The entity filter: a rule naming `entity` alone leaves `other` mixed; a plain `all()`
        // filter skips the properties subtree; no filter covers every entity, properties included,
        // and still only the columns its selector matches.
        let entity_only = type_rule
            .clone()
            .with_entity_filter(EntityPathFilter::parse_forgiving("+ /entity"));
        let units = units_of(vec![entity_only]);
        assert!(units.contains(&only_run(&view, &[1], colors(), target)));
        assert!(units.contains(&merge_split_run(&view, &[2], &global)));
        assert!(units.contains(&merge_split_run(&view, &[3], &global)));

        let plain_all = type_rule
            .clone()
            .with_entity_filter(EntityPathFilter::all());
        let units = units_of(vec![plain_all]);
        assert!(units.contains(&only_run(&view, &[1], colors(), target)));
        assert!(units.contains(&merge_split_run(&view, &[3], &global)));

        assert_eq!(type_rule.entity_filter, None);
        let units = units_of(vec![type_rule.clone()]);
        assert!(units.contains(&only_run(&view, &[1], colors(), target)));
        assert!(units.contains(&only_run(&view, &[2], colors(), target)));
        assert!(units.contains(&only_run(&view, &[3], colors(), target)));
        for id in [1, 2, 3] {
            assert!(units.contains(&PlanUnit::MergeSplitRun {
                inputs: vec![ChunkSlice::except(idx_of(&view, id), color_set())],
                target,
            }));
        }
        assert!(units.contains(&merge_split_run(&view, &[4], &global)));

        // Overrides, under a global target and without one.
        let custom_target = merge_split_settings(64, 0);
        let with_override = |merge_split: MergeSplitOverride| OwnChunkRule {
            merge_split,
            ..type_rule.clone()
        };
        let colors_run_of = |settings: &OptimizationSettings, rule: OwnChunkRule| {
            let mut units = colors_units(settings, vec![rule]);
            assert_eq!(units.len(), 1);
            units.pop().unwrap()
        };
        let disabled = OptimizationSettings {
            merge_split: None,
            ..global.clone()
        };

        assert_eq!(
            colors_run_of(&global, with_override(MergeSplitOverride::Inherit)),
            only_run(&view, &[1], colors(), target)
        );
        assert_eq!(
            colors_run_of(&disabled, with_override(MergeSplitOverride::Inherit)),
            PlanUnit::Passthrough(ChunkSlice::only(idx_of(&view, 1), color_set()))
        );
        for settings in [&global, &disabled] {
            assert_eq!(
                colors_run_of(settings, with_override(MergeSplitOverride::Passthrough)),
                PlanUnit::Passthrough(ChunkSlice::only(idx_of(&view, 1), color_set()))
            );
            assert_eq!(
                colors_run_of(
                    settings,
                    with_override(MergeSplitOverride::MergeSplit(custom_target))
                ),
                only_run(&view, &[1], colors(), custom_target)
            );
        }
    }

    /// Static and orphan chunks split on their own columns into passthroughs, and never merge.
    #[test]
    fn static_and_orphan_chunks_split() {
        let rules = vec![OwnChunkRule::new(ColumnSelector::Type(MyColor::name()))];

        let chunks = vec![static_mixed_chunk(1, "mixed"), static_chunk(2, "points")];
        let view = view_of(&chunks);
        let units = plan(&view, &with_rules(settings(1024 * 1024, 0), rules.clone()));
        assert!(plan_covers_view(&view, &units));
        assert_eq!(
            units,
            vec![
                PlanUnit::Passthrough(ChunkSlice::only(idx_of(&view, 1), color_set())),
                PlanUnit::Passthrough(ChunkSlice::except(idx_of(&view, 1), [colors()])),
                passthrough_all(&view, 2),
            ]
        );

        // An orphan mixed chunk: its `:start` columns are null, so the index records no column
        // for it and it passes through whole.
        let chunks = [
            temporal_mixed_chunk(1, "entity", &[0, 1], 4, 4),
            temporal_mixed_chunk(2, "entity", &[10, 11], 4, 4),
        ];
        let view = view_with_orphan(&chunks);
        let settings = with_rules(settings(1024 * 1024, 0), rules);
        let units = plan(&view, &settings);
        assert!(plan_covers_view(&view, &units));
        assert_eq!(
            units,
            vec![
                only_run(&view, &[1], colors(), settings.merge_split.unwrap()),
                PlanUnit::MergeSplitRun {
                    inputs: vec![ChunkSlice::except(idx_of(&view, 1), color_set())],
                    target: settings.merge_split.unwrap(),
                },
                passthrough_all(&view, 2),
            ]
        );
    }

    /// Hand-built plans that break the completeness invariant are rejected.
    #[test]
    fn plan_covers_view_rejects() {
        let chunks = vec![temporal_mixed_chunk(1, "entity", &[0, 1], 4, 4)];
        let view = view_of(&chunks);
        let idx = idx_of(&view, 1);
        let points = MyPoints::descriptor_points().component;
        let target = merge_split_settings(1024, 0);

        let passthroughs = |slices: Vec<ChunkSlice>| -> Vec<PlanUnit> {
            slices.into_iter().map(PlanUnit::Passthrough).collect()
        };

        // The valid forms, for contrast.
        assert!(plan_covers_view(
            &view,
            &passthroughs(vec![ChunkSlice::all(idx)])
        ));
        assert!(plan_covers_view(
            &view,
            &passthroughs(vec![
                ChunkSlice::only(idx, color_set()),
                ChunkSlice::except(idx, color_set()),
            ])
        ));
        assert!(plan_covers_view(
            &view,
            &passthroughs(vec![
                ChunkSlice::only(idx, color_set()),
                ChunkSlice::only(idx, [points]),
            ])
        ));

        // An `Except` set that differs from the `Only`s.
        assert!(!plan_covers_view(
            &view,
            &passthroughs(vec![
                ChunkSlice::only(idx, color_set()),
                ChunkSlice::except(idx, [points]),
            ])
        ));
        // An empty `Except` set.
        assert!(!plan_covers_view(
            &view,
            &passthroughs(vec![ChunkSlice::except(idx, [])])
        ));
        // Two `Only`s of one column.
        assert!(!plan_covers_view(
            &view,
            &passthroughs(vec![
                ChunkSlice::only(idx, color_set()),
                ChunkSlice::only(idx, color_set()),
                ChunkSlice::except(idx, color_set()),
            ])
        ));
        // Overlapping `Only` sets.
        assert!(!plan_covers_view(
            &view,
            &passthroughs(vec![
                ChunkSlice::only(idx, [colors(), points]),
                ChunkSlice::only(idx, [points]),
            ])
        ));
        // An empty `Only` set.
        assert!(!plan_covers_view(
            &view,
            &passthroughs(vec![ChunkSlice::only(idx, []), ChunkSlice::all(idx)])
        ));
        // `Only`s without `Except` whose set differs from the chunk's columns.
        assert!(!plan_covers_view(
            &view,
            &passthroughs(vec![ChunkSlice::only(idx, color_set())])
        ));
        // `All` next to another slice.
        assert!(!plan_covers_view(
            &view,
            &passthroughs(vec![
                ChunkSlice::all(idx),
                ChunkSlice::only(idx, color_set())
            ])
        ));
        // A chunk in no unit.
        assert!(!plan_covers_view(&view, &[]));
        // An empty run.
        assert!(!plan_covers_view(
            &view,
            &[
                PlanUnit::MergeSplitRun {
                    inputs: vec![],
                    target,
                },
                PlanUnit::Passthrough(ChunkSlice::all(idx)),
            ]
        ));
    }
}
