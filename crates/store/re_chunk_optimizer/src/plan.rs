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

use indexmap::IndexMap;

use re_chunk::{ComponentIdentifier, ComponentType};
use re_log_types::{
    EntityPath, EntityPathFilter, ResolvedEntityPathFilter, Timeline, TimelineName,
};
use re_sdk_types::archetypes::VideoStream;
use vec1::Vec1;

use crate::row_merge::MergeInput;
use crate::settings::{
    ColumnSelector, GopBatchingRule, KeyframePlacement, MergeSplitOverride, MergeSplitSettings,
    OptimizationSettings, OwnChunkRule,
};
use crate::view::{ChunkIdx, ChunkIndexView, ChunkMeta, EntityView, TimelineSetGroup};

/// A column subset of one chunk of the index.
// Note: column only for now, will eventually extend to row windows as well.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkSlice {
    pub chunk: ChunkIdx,
    pub columns: ColumnSelection,
}

/// This enables the use of shared utilities such as
/// [`load_in_order`](crate::executor::load_in_order).
impl AsRef<Self> for ChunkSlice {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
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

impl ColumnSelection {
    /// The columns of `meta`'s chunk that this selection keeps.
    fn apply(&self, meta: &ChunkMeta) -> BTreeSet<ComponentIdentifier> {
        match self {
            Self::All => meta.components.keys().copied().collect(),
            Self::Only(columns) => columns.clone(),
            Self::Except(excluded) => meta
                .components
                .keys()
                .filter(|column| !excluded.contains(column))
                .copied()
                .collect(),
        }
    }
}

/// How a GoP run cuts its samples into output chunks. A GoP is never split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GopChunkingConfig {
    /// One chunk per GoP.
    OnePerGop,

    /// Group GoPs toward a target chunk size.
    Packed(MergeSplitSettings),
}

/// Where a GoP run puts the `is_keyframe` column it generates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyframePlacementConfig {
    /// Collocated in the same chunk as the sample column.
    Inline,

    /// Chunks of its own, packed toward the target.
    OwnChunk(MergeSplitSettings),
}

/// The `is_keyframe` target when neither the rule nor the global settings give one: one chunk per
/// stream.
pub const KEYFRAMES_UNBOUNDED: MergeSplitSettings = MergeSplitSettings {
    max_bytes: std::num::NonZeroU64::MAX,
    max_rows: None,
    max_rows_if_unsorted: None,
};

/// One piece of the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanUnit {
    /// Emit the slice as-is.
    Passthrough(ChunkSlice),

    /// Discard the slice.
    ///
    /// Essentially a no-op, kept only to assess plan coverage completeness.
    Drop(ChunkSlice),

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

    /// Decide the stream's codec, sort the inputs' rows by `timeline`, cut at detected keyframes,
    /// pack whole GoPs, and generate the `is_keyframe` column.
    GopRun {
        entity_path: EntityPath,

        /// Slices used to determine the stream's codec.
        ///
        /// These are read-only, so not used to assess plan completeness. A single static codec chunk
        /// is produced instead.
        codec: CodecSource,

        /// One per sample-carrying chunk of the entity, in range-start order on `timeline`. Each
        /// slice is the chunk's `VideoStream:sample` column, plus its `VideoStream:codec` column,
        /// if any, when the stream's codec is temporal (see [`CodecSource::Temporal`]): the run
        /// checks each of those codec cells against the stream's codec.
        inputs: Vec<MergeInput>,

        timeline: TimelineName,
        gop_chunking: GopChunkingConfig,
        keyframes: KeyframePlacementConfig,
    },
}

/// The slices a GoP run decides its stream's codec from.
///
/// Either way, the stream comes out with one static codec and no temporal one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecSource {
    /// Every static codec slice of the entity. The latest row wins, and shadows every temporal
    /// codec cell.
    Static(Vec1<ChunkSlice>),

    /// Every temporal codec slice outside the sample chunks, or else the first sample chunk's.
    /// Every temporal codec cell must hold the same codec.
    Temporal(Vec1<ChunkSlice>),
}

pub fn sample_column() -> ComponentIdentifier {
    VideoStream::descriptor_sample().component
}

pub fn codec_column() -> ComponentIdentifier {
    VideoStream::descriptor_codec().component
}

pub fn keyframe_column() -> ComponentIdentifier {
    VideoStream::descriptor_is_keyframe().component
}

/// The own columns of one entity, each with the target its run merges toward, or `None` when its
/// slices pass through.
type OwnColumns = BTreeMap<ComponentIdentifier, Option<MergeSplitSettings>>;

/// The GoP rule of one matched entity, resolved.
struct GopEntity<'a> {
    /// The entity's only timeline set with sample-carrying chunks.
    sample_group: &'a TimelineSetGroup,

    codec: CodecSource,
    gop_chunking: GopChunkingConfig,
    keyframes: KeyframePlacementConfig,
}

/// Build a plan.
pub fn plan(view: &ChunkIndexView, settings: &OptimizationSettings) -> Vec<PlanUnit> {
    re_tracing::profile_function!();

    let mut units = Vec::new();
    let mut claimed = vec![false; view.num_chunks()];

    // The GoP rule owns the sample, codec, and keyframe columns of the entities it matches.
    let gop_entities = resolve_gop_entities(view, settings);
    let mut own_columns_by_entity = resolve_own_columns(view, settings);
    for entity_path in gop_entities.keys() {
        if let Some(own) = own_columns_by_entity.get_mut(entity_path) {
            own.remove(&sample_column());
            own.remove(&codec_column());
            own.remove(&keyframe_column());
        }
    }
    let no_own_columns = OwnColumns::new();
    let own_columns_of = |entity_path: &EntityPath| {
        own_columns_by_entity
            .get(entity_path)
            .unwrap_or(&no_own_columns)
    };

    // Static chunks pass through, in chunk-index order, split on their own columns. A matched
    // entity's static `is_keyframe` is left to its GoP run.
    let static_keyframe = [keyframe_column()];
    for (idx, meta) in view.chunks() {
        if meta.is_static {
            claimed[idx.as_usize()] = true;
            let excluded_columns: &[ComponentIdentifier] =
                if gop_entities.contains_key(&meta.entity_path) {
                    &static_keyframe
                } else {
                    &[]
                };
            units.extend(
                slices_of_chunk(
                    idx,
                    meta,
                    own_columns_of(&meta.entity_path),
                    excluded_columns,
                )
                .into_iter()
                .map(PlanUnit::Passthrough),
            );
        }
    }

    // Temporal chunks merge per entity and per exact timeline set — the same grouping the merge
    // gate `Chunk::concatenable` enforces at merge time. Within a group, every own column gets a
    // run of its own over the chunks that carry it, then the rest of the chunks form runs by
    // column set (see [`partition_rest`]). A matched entity's video columns are left to its GoP
    // run.
    let video_columns = [sample_column(), codec_column(), keyframe_column()];
    for (entity_path, entity) in &view.entities {
        let own = own_columns_of(entity_path);
        let gop = gop_entities.get(entity_path);

        let already_handled_columns: &[ComponentIdentifier] = if let Some(gop) = gop {
            plan_gop_entity(view, entity_path, entity, gop, settings, &mut units);
            &video_columns
        } else {
            &[]
        };

        for group in &entity.timeline_sets {
            plan_group(
                view,
                group,
                own,
                already_handled_columns,
                settings,
                &mut units,
                &mut claimed,
            );
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
                slices_of_chunk(idx, meta, own_columns_of(&meta.entity_path), &[])
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

/// Plan one group: own runs, then rest runs, in sweep order.
///
/// The `excluded_columns` stay out of both; the caller routes them through other units.
fn plan_group(
    view: &ChunkIndexView,
    group: &TimelineSetGroup,
    own: &OwnColumns,
    excluded_columns: &[ComponentIdentifier],
    settings: &OptimizationSettings,
    units: &mut Vec<PlanUnit>,
    claimed: &mut [bool],
) {
    let order = sweep_order(view, group, settings.target_timeline.as_ref());
    for &idx in &order {
        claimed[idx.as_usize()] = true;
    }

    let mut own_runs: BTreeMap<ComponentIdentifier, Vec<ChunkSlice>> = BTreeMap::new();
    let mut rest = Vec::new();
    for &idx in &order {
        let meta = view.chunk(idx);
        let mut present = present_own_columns(meta, own);
        for &column in &present {
            own_runs
                .entry(column)
                .or_default()
                .push(ChunkSlice::only(idx, [column]));
        }
        present.extend(
            excluded_columns
                .iter()
                .filter(|column| meta.components.contains_key(column))
                .copied(),
        );
        rest.extend(rest_slice(idx, meta, present));
    }

    for (column, inputs) in own_runs {
        emit(units, inputs, own[&column]);
    }
    for inputs in partition_into_run(view, rest) {
        emit(units, inputs, settings.merge_split);
    }
}

/// Plan a matched entity's video columns: `VideoStream:sample`, `VideoStream:codec`, and
/// `VideoStream:is_keyframe`.
///
/// # Samples
///
/// The sample slices of one entity are gathered into a single [`PlanUnit::GopRun`]. They must
/// share one timeline set. If they don't, GoP batching of the entity is skipped with a warning
/// (enforced by [`resolve_gop_entities`]). The run detects keyframes from the samples, cuts at
/// them, and packs whole GoPs per [`GopChunkingConfig`].
///
/// # Keyframes
///
/// The input `is_keyframe` column is entirely dropped, on any timeline set: the run regenerates the
/// column from detection, placed per [`KeyframePlacementConfig`].
///
/// # Codec
///
/// The stream must have a single codec, and comes out with it as one static cell (see
/// [`CodecSource`]).
///
/// - With a static codec, the latest one decides and shadows a temporal codec column, which is
///   dropped unchecked.
/// - Without one, all temporal codec values must be the same. Those stored outside the sample
///   chunks, on any timeline set, are read before the run's first output, then dropped. Those
///   inside the sample chunks ride along in the run's inputs: the run checks them as it loads
///   those chunks, then drops them (reading them up front would load every sample chunk twice).
///   With no codec values outside the sample chunks, the first sample chunk's codec column is read
///   up front instead, to decide the codec before the first keyframe detection.
///
/// A codec switch is unsupported: it fails the whole optimization.
fn plan_gop_entity(
    view: &ChunkIndexView,
    entity_path: &EntityPath,
    entity: &EntityView,
    gop: &GopEntity<'_>,
    settings: &OptimizationSettings,
    units: &mut Vec<PlanUnit>,
) {
    let carries = |idx: ChunkIdx, column: ComponentIdentifier| {
        view.chunk(idx).components.contains_key(&column)
    };
    let run_checks_codec = |idx: ChunkIdx| {
        matches!(gop.codec, CodecSource::Temporal(_)) && carries(idx, sample_column())
    };

    // A static `is_keyframe` would shadow the regenerated, temporal one.
    let static_keyframes: Vec<ChunkIdx> = entity
        .static_chunks
        .iter()
        .copied()
        .filter(|&idx| carries(idx, keyframe_column()))
        .collect();
    if !static_keyframes.is_empty() {
        re_log::warn!(
            "Dropping the static {} column of a video stream, which would hide the regenerated \
             keyframes\nEntity: {entity_path}",
            keyframe_column()
        );
    }
    for idx in static_keyframes {
        units.push(PlanUnit::Drop(ChunkSlice::only(idx, [keyframe_column()])));
    }

    for idx in temporal_chunks_in_index_order(entity) {
        if carries(idx, keyframe_column()) {
            units.push(PlanUnit::Drop(ChunkSlice::only(idx, [keyframe_column()])));
        }
        if carries(idx, codec_column()) && !run_checks_codec(idx) {
            units.push(PlanUnit::Drop(ChunkSlice::only(idx, [codec_column()])));
        }
    }

    let group = gop.sample_group;
    let timeline = run_timeline(group, settings.target_timeline.as_ref());
    let inputs = MergeInput::from_group(group, &timeline, |idx| {
        carries(idx, sample_column()).then(|| {
            let columns = std::iter::chain(
                [sample_column()],
                (run_checks_codec(idx) && carries(idx, codec_column())).then(codec_column),
            );
            ChunkSlice::only(idx, columns)
        })
    });
    units.push(PlanUnit::GopRun {
        entity_path: entity_path.clone(),
        codec: gop.codec.clone(),
        inputs,
        timeline: *timeline.name(),
        gop_chunking: gop.gop_chunking,
        keyframes: gop.keyframes,
    });
}

/// Whether any chunk of the group carries a `VideoStream:sample` column.
fn carries_samples(view: &ChunkIndexView, group: &TimelineSetGroup) -> bool {
    group
        .per_timeline
        .values()
        .next()
        .into_iter()
        .flatten()
        .any(|span| {
            view.chunk(span.chunk)
                .components
                .contains_key(&sample_column())
        })
}

/// The timeline a GoP run sorts its rows on: the target timeline when the group has it, else the
/// best by [`Timeline::pick_best_timeline`]'s priority order.
fn run_timeline(group: &TimelineSetGroup, target_timeline: Option<&TimelineName>) -> Timeline {
    if let Some(target) = target_timeline
        && let Some(timeline) = group.timelines.iter().find(|t| t.name() == target)
    {
        return *timeline;
    }
    // Every row of a group is on every timeline of the group, so no score tells them apart.
    Timeline::pick_best_timeline(group.timelines.iter(), |_| 0)
}

/// Split a group's rest slices into merge runs, so that only slices sharing columns merge.
///
/// A merge null-pads every column that some of its inputs lack. Merging slices that share no
/// column would produce mixed chunks that are mostly nulls, and make a reader of one stream fetch
/// the other stream's bytes too. So the slices are grouped by column set (the columns a slice
/// carries), and a run only gathers sets that are linked through shared columns:
///
/// - Distinct column sets are taken heaviest first (ties in sweep order), each weighed by the total
///   rows of its slices. Rows, not bytes: the index has no per-column sizes.
/// - A set joins the heaviest run it shares a column with, and the run takes on its columns.
/// - A set sharing no column with any run starts a new run.
///
/// A set overlapping several runs (e.g. a few chunks that carry two otherwise separate streams)
/// joins the heaviest one rather than fusing them. The streams therefore stay apart only as long as
/// such bridging sets are lighter than both streams.
///
/// Returns the runs in order of their first slice, each in sweep order.
fn partition_into_run(view: &ChunkIndexView, rest: Vec<ChunkSlice>) -> Vec<Vec<ChunkSlice>> {
    // 1. Extract the columns and weight (aka row count) of every slice.
    let slice_columns: Vec<(BTreeSet<ComponentIdentifier>, u64)> = rest
        .iter()
        .map(|slice| {
            let meta = view.chunk(slice.chunk);
            (slice.columns.apply(meta), meta.num_rows)
        })
        .collect();

    // 2. Based on their columns, assign every slice to a run.
    let run_of_slice = assign_runs(&slice_columns);

    // 3. Collect the slices into their assigned runs, in order of first slice.
    let mut runs: IndexMap<usize, Vec<ChunkSlice>> = IndexMap::new();
    for (slice, run) in std::iter::zip(rest, run_of_slice) {
        runs.entry(run).or_default().push(slice);
    }
    runs.into_values().collect()
}

/// Assign slices, in sweep order, to merge runs by column set, as described in
/// [`partition_into_run`].
///
/// Each slice comes as its columns and its weight. Slices with equal columns always land in the
/// same run.
///
/// Returns the run of each slice.
fn assign_runs(slice_columns: &[(BTreeSet<ComponentIdentifier>, u64)]) -> Vec<usize> {
    // 1. Gather the distinct column sets, in order of first slice, each weighed by all its slices.
    let mut sets: IndexMap<&BTreeSet<ComponentIdentifier>, u64> = IndexMap::new();
    let set_of_slice: Vec<usize> = slice_columns
        .iter()
        .map(|(columns, weight)| {
            let entry = sets.entry(columns);
            let set = entry.index();
            let total = entry.or_insert(0);
            *total = total.saturating_add(*weight);
            set
        })
        .collect();

    let mut by_weight: Vec<usize> = (0..sets.len()).collect();
    by_weight.sort_by_key(|&set| (std::cmp::Reverse(sets[set]), set));

    // 2. Dispatch sets into runs.
    struct Run {
        /// Merged schema of all the sets included in this run.
        columns: BTreeSet<ComponentIdentifier>,

        weight: u64,
    }

    let mut runs: Vec<Run> = Vec::new();
    let mut run_of_set = vec![0; sets.len()];
    for set in by_weight {
        let (&columns, &weight) = sets.get_index(set).expect("`set` indexes `sets`");
        let heaviest_overlapping = runs
            .iter()
            .enumerate()
            .filter(|(_, run)| !run.columns.is_disjoint(columns))
            // Ties go to the earliest run.
            .max_by_key(|&(i, run)| (run.weight, std::cmp::Reverse(i)))
            .map(|(i, _)| i);
        let run = heaviest_overlapping.unwrap_or_else(|| {
            runs.push(Run {
                columns: BTreeSet::new(),
                weight: 0,
            });
            runs.len() - 1
        });
        runs[run].columns.extend(columns);
        runs[run].weight = runs[run].weight.saturating_add(weight);
        run_of_set[set] = run;
    }

    set_of_slice
        .into_iter()
        .map(|set| run_of_set[set])
        .collect()
}

fn emit(units: &mut Vec<PlanUnit>, inputs: Vec<ChunkSlice>, target: Option<MergeSplitSettings>) {
    match target {
        Some(target) => units.push(PlanUnit::MergeSplitRun { inputs, target }),
        None => units.extend(inputs.into_iter().map(PlanUnit::Passthrough)),
    }
}

/// The slices of one chunk: one per own column it carries, then the rest. The `excluded_columns`
/// are in neither; the caller routes them through other units.
fn slices_of_chunk(
    idx: ChunkIdx,
    meta: &ChunkMeta,
    own: &OwnColumns,
    excluded_columns: &[ComponentIdentifier],
) -> Vec<ChunkSlice> {
    let mut present = present_own_columns(meta, own);
    let mut slices: Vec<ChunkSlice> = present
        .iter()
        .map(|&column| ChunkSlice::only(idx, [column]))
        .collect();
    present.extend(
        excluded_columns
            .iter()
            .filter(|column| meta.components.contains_key(column))
            .copied(),
    );
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

/// Resolve [`OptimizationSettings::gop_batching`] per entity.
///
/// See [`plan_gop_entity`] for details.
fn resolve_gop_entities<'a>(
    view: &'a ChunkIndexView,
    settings: &OptimizationSettings,
) -> BTreeMap<EntityPath, GopEntity<'a>> {
    if settings.gop_batching.is_empty() {
        return BTreeMap::new();
    }

    let rules: Vec<(Option<ResolvedEntityPathFilter>, &GopBatchingRule)> = settings
        .gop_batching
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

    let resolve_chunking = |samples: MergeSplitOverride| match samples {
        MergeSplitOverride::Inherit => settings
            .merge_split
            .map_or(GopChunkingConfig::OnePerGop, GopChunkingConfig::Packed),
        MergeSplitOverride::Passthrough => GopChunkingConfig::OnePerGop,
        MergeSplitOverride::MergeSplit(target) => GopChunkingConfig::Packed(target),
    };

    view.entities
        .iter()
        .filter_map(|(entity_path, entity)| {
            let (_, rule) = rules.iter().find(|(filter, _)| {
                filter
                    .as_ref()
                    .is_none_or(|filter| filter.matches(entity_path))
            })?;

            let temporal = temporal_chunks_in_index_order(entity);
            let carries = |idx: &ChunkIdx, column: ComponentIdentifier| {
                view.chunk(*idx).components.contains_key(&column)
            };
            // Chunks on different timeline sets never merge, so samples spread over several
            // could not form one stream.
            let mut sample_groups = entity
                .timeline_sets
                .iter()
                .filter(|group| carries_samples(view, group));
            let sample_group = match (sample_groups.next(), sample_groups.next()) {
                (None, _) => return None,
                (Some(group), None) => group,
                (Some(_), Some(_)) => {
                    re_log::warn!(
                        "Skipping GoP batching of a video stream whose samples span several \
                         timeline sets\nEntity: {entity_path}"
                    );
                    return None;
                }
            };
            let codec_slice = |idx: &ChunkIdx| ChunkSlice::only(*idx, [codec_column()]);
            let static_codecs: Vec<ChunkSlice> = entity
                .static_chunks
                .iter()
                .filter(|idx| carries(idx, codec_column()))
                .map(codec_slice)
                .collect();
            let codec = if let Ok(static_codecs) = Vec1::try_from_vec(static_codecs) {
                CodecSource::Static(static_codecs)
            } else {
                let mut temporal_codecs: Vec<ChunkSlice> = temporal
                    .iter()
                    .filter(|idx| carries(idx, codec_column()) && !carries(idx, sample_column()))
                    .map(codec_slice)
                    .collect();
                if temporal_codecs.is_empty() {
                    temporal_codecs.extend(
                        temporal
                            .iter()
                            .find(|idx| carries(idx, codec_column()))
                            .map(codec_slice),
                    );
                }
                let Ok(temporal_codecs) = Vec1::try_from_vec(temporal_codecs) else {
                    re_log::warn!(
                        "Skipping GoP batching of a video stream without a {} column\nEntity: {entity_path}",
                        codec_column()
                    );
                    return None;
                };
                CodecSource::Temporal(temporal_codecs)
            };

            let keyframes = match rule.keyframes {
                KeyframePlacement::Inline => KeyframePlacementConfig::Inline,
                KeyframePlacement::OwnChunk(target) => KeyframePlacementConfig::OwnChunk(
                    target.or(settings.merge_split).unwrap_or(KEYFRAMES_UNBOUNDED),
                ),
            };
            Some((
                entity_path.clone(),
                GopEntity {
                    sample_group,
                    codec,
                    gop_chunking: resolve_chunking(rule.samples),
                    keyframes,
                },
            ))
        })
        .collect()
}

fn temporal_chunks_in_index_order(entity: &EntityView) -> Vec<ChunkIdx> {
    let mut chunks: Vec<ChunkIdx> = entity
        .timeline_sets
        .iter()
        .flat_map(|group| {
            group
                .per_timeline
                .values()
                .next()
                .into_iter()
                .flatten()
                .map(|span| span.chunk)
        })
        .collect();
    chunks.sort_unstable();
    chunks
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
        for (&column, component_type) in meta.components.iter() {
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
            PlanUnit::Passthrough(slice) | PlanUnit::Drop(slice) => {
                slices[slice.chunk.as_usize()].push(&slice.columns);
            }

            PlanUnit::MergeSplitRun { inputs, .. } => {
                if inputs.is_empty() {
                    return false;
                }
                for slice in inputs {
                    slices[slice.chunk.as_usize()].push(&slice.columns);
                }
            }

            PlanUnit::GopRun { inputs, .. } => {
                if inputs.is_empty() {
                    return false;
                }
                for MergeInput { slice, .. } in inputs {
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
    use re_chunk_index::RawRrdManifest;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{EntityPathFilter, StoreId, StoreKind, TimePoint, Timeline};
    use re_sdk_types::archetypes::VideoStream;
    use re_sdk_types::components::{IsKeyframe, VideoCodec, VideoSample};
    use re_types_core::{Component as _, ComponentBatch as _, ComponentDescriptor};

    use super::{
        ChunkSlice, CodecSource, GopChunkingConfig, KeyframePlacementConfig, PlanUnit,
        codec_column, keyframe_column, plan, plan_covers_view, sample_column,
    };
    use crate::view::{ChunkIdx, ChunkIndexView};
    use crate::{
        ColumnSelector, GopBatchingRule, KeyframePlacement, MergeSplitOverride, MergeSplitSettings,
        OptimizationSettings, OwnChunkRule,
    };

    /// A temporal chunk on the `frame` timeline: one row per time, `points_per_row` points each.
    fn temporal_chunk(id: u128, entity: &str, times: &[i64], points_per_row: u32) -> Chunk {
        let frame = Timeline::new_sequence("frame");
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for &time in times {
            builder = builder.with_serialized_batches(
                RowId::new(),
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
        for &time in times {
            builder = builder.with_serialized_batches(
                RowId::new(),
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
        for &time in times {
            builder = builder.with_serialized_batches(
                RowId::new(),
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
        for &time in times {
            builder = builder.with_serialized_batches(
                RowId::new(),
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

    /// A temporal chunk with one point under each of `descriptors` on every row.
    fn temporal_chunk_with(
        id: u128,
        entity: &str,
        times: &[i64],
        descriptors: &[&ComponentDescriptor],
    ) -> Chunk {
        let frame = Timeline::new_sequence("frame");
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for (i, &time) in times.iter().enumerate() {
            builder = builder.with_serialized_batches(
                RowId::from_u128((id << 32) + i as u128 + 1),
                [(frame, time)],
                descriptors.iter().map(|descriptor| {
                    MyPoint::from_iter(0..1)
                        .try_serialized((*descriptor).clone())
                        .unwrap()
                }),
            );
        }
        builder.build().unwrap()
    }

    fn static_chunk(id: u128, entity: &str) -> Chunk {
        Chunk::builder_with_id(ChunkId::from_u128(id), entity)
            .with_serialized_batches(
                RowId::new(),
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
                RowId::new(),
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
            gop_batching: Vec::new(),
        }
    }

    fn with_rules(
        mut settings: OptimizationSettings,
        rules: Vec<OwnChunkRule>,
    ) -> OptimizationSettings {
        settings.own_chunk = rules;
        settings
    }

    /// A group's chunks of one column set form one merge/split run in sweep order, whatever the
    /// byte target — output boundaries and splits belong to the executor.
    #[test]
    fn group_forms_one_run() {
        let chunks: Vec<Chunk> = (0..7)
            .map(|i| temporal_chunk(i + 1, "entity", &[i as i64 * 10, i as i64 * 10 + 1], 64))
            .collect();
        let view = view_of(&chunks);

        let settings = settings(1, 0); // even a tiny target: the plan shape does not change
        let units = plan(&view, &settings);

        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1 B)
              #1 all
              #2 all
              #3 all
              #4 all
              #5 all
              #6 all
              #7 all
            ");
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
                    RowId::new(),
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

        let units = plan(&view, &settings(u64::MAX / 2, 0));

        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (9223372036854775807 B)
              #1 all
              #2 all
            merge_split (9223372036854775807 B)
              #3 all
            merge_split (9223372036854775807 B)
              #4 all
            ");
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
        let units = plan(&view, &settings(1, 0));
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #1 all
            merge_split (1 B)
              #2 all
              #3 all
            ");

        // `merge_split: None`: every chunk stands alone, no run.
        let disabled = OptimizationSettings {
            merge_split: None,
            target_timeline: None,
            own_chunk: Vec::new(),
            gop_batching: Vec::new(),
        };
        let units = plan(&view, &disabled);
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #1 all
            passthrough #2 all
            passthrough #3 all
            ");
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

        let plan_with_target = |name: Option<&str>| {
            let settings = OptimizationSettings {
                target_timeline: name
                    .map(|name| re_log_types::TimelineName::try_new(name).unwrap()),
                ..settings(1024 * 1024, 0)
            };
            let units = plan(&view, &settings);
            assert!(plan_covers_view(&view, &units));
            render_plan(&view, &units)
        };

        // Time order sweeps 0, 10, 20, 30.
        insta::assert_snapshot!(plan_with_target(Some("frame")), @r"
            merge_split (1048576 B)
              #1 all
              #3 all
              #2 all
              #4 all
            ");

        // File order sweeps 0, 20, 10, 30 — both with no target and with an unknown one.
        insta::assert_snapshot!(plan_with_target(None), @r"
            merge_split (1048576 B)
              #1 all
              #2 all
              #3 all
              #4 all
            ");
        insta::assert_snapshot!(plan_with_target(Some("no_such_timeline")), @r"
            merge_split (1048576 B)
              #1 all
              #2 all
              #3 all
              #4 all
            ");
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
        let units = plan(&view, &settings(1024 * 1024, 0));

        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 all
            passthrough #2 all
            ");
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
                    RowId::new(),
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
        let units = plan(&view, &settings);

        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
              #3 only[example.MyPoints:colors]
              #4 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #1 except[example.MyPoints:colors]
              #2 all
              #4 except[example.MyPoints:colors]
            merge_split (1048576 B)
              #5 only[example.MyPoints:colors]
              #6 only[example.MyPoints:colors]
            ");
    }

    /// The rest of a group splits into runs by column set: chunks sharing no column never share a
    /// run, chunks whose columns overlap do, and runs come out in order of their first chunk, not
    /// by weight.
    #[test]
    fn rest_runs_follow_shared_columns() {
        let points = MyPoints::descriptor_points();
        let colors = MyPoints::descriptor_colors();
        let custom = untyped_descriptor();
        // The custom run has more rows, but its first chunk comes second.
        let chunks = vec![
            temporal_chunk_with(1, "entity", &[0, 1], &[&points]),
            temporal_chunk_with(2, "entity", &[2, 3, 4, 5], &[&custom]),
            temporal_chunk_with(3, "entity", &[6, 7], &[&points, &colors]),
            temporal_chunk_with(4, "entity", &[8, 9, 10, 11], &[&custom]),
            temporal_chunk_with(5, "entity", &[12, 13], &[&points]),
        ];
        let view = view_of(&chunks);
        let settings = settings(1024 * 1024, 0);
        let units = plan(&view, &settings);
        assert!(plan_covers_view(&view, &units));

        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 all
              #3 all
              #5 all
            merge_split (1048576 B)
              #2 all
              #4 all
            ");
    }

    /// A column set overlapping two runs, lighter than both, joins the one with the most rows,
    /// whichever stream that is, instead of fusing them.
    #[test]
    fn bridge_joins_the_heaviest_run() {
        let points = MyPoints::descriptor_points();
        let custom = untyped_descriptor();
        let bridged = |points_rows: i64, custom_rows: i64| {
            let times = |start: i64, rows: i64| (start..start + rows).collect::<Vec<_>>();
            vec![
                temporal_chunk_with(1, "entity", &[0], &[&points, &custom]),
                temporal_chunk_with(2, "entity", &times(10, points_rows), &[&points]),
                temporal_chunk_with(3, "entity", &times(10, custom_rows), &[&custom]),
                temporal_chunk_with(4, "entity", &times(20, points_rows), &[&points]),
                temporal_chunk_with(5, "entity", &times(20, custom_rows), &[&custom]),
            ]
        };
        let settings = settings(1024 * 1024, 0);

        let chunks = bridged(4, 2);
        let view = view_of(&chunks);
        let units = plan(&view, &settings);
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 all
              #2 all
              #4 all
            merge_split (1048576 B)
              #3 all
              #5 all
            ");

        let chunks = bridged(2, 4);
        let view = view_of(&chunks);
        let units = plan(&view, &settings);
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 all
              #3 all
              #5 all
            merge_split (1048576 B)
              #2 all
              #4 all
            ");
    }

    /// The rest runs are formed from the rest slices' columns: an own column links nothing.
    #[test]
    fn own_columns_do_not_link_rest_runs() {
        let points = MyPoints::descriptor_points();
        let colors = MyPoints::descriptor_colors();
        let custom = untyped_descriptor();
        let chunks = vec![
            temporal_chunk_with(1, "entity", &[0, 1], &[&points, &colors]),
            temporal_chunk_with(2, "entity", &[2, 3], &[&custom, &colors]),
            temporal_chunk_with(3, "entity", &[4, 5], &[&points]),
            temporal_chunk_with(4, "entity", &[6, 7], &[&custom]),
        ];
        let view = view_of(&chunks);

        // Without rules, colors links chunks 1 and 2, and with them everything.
        let global = settings(1024 * 1024, 0);
        let units = plan(&view, &global);
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 all
              #2 all
              #3 all
              #4 all
            ");

        // With colors as an own column, the rest of chunks 1 and 2 share nothing.
        let settings = with_rules(
            global.clone(),
            vec![OwnChunkRule::new(ColumnSelector::Column(colors.component))],
        );
        let units = plan(&view, &settings);
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
              #2 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #1 except[example.MyPoints:colors]
              #3 all
            merge_split (1048576 B)
              #2 except[example.MyPoints:colors]
              #4 all
            ");
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
            let units: Vec<PlanUnit> = units
                .into_iter()
                .filter(|unit| match unit {
                    PlanUnit::Passthrough(slice) => is_colors_of_1(slice),
                    PlanUnit::MergeSplitRun { inputs, .. } => inputs.iter().any(is_colors_of_1),
                    PlanUnit::Drop(_) | PlanUnit::GopRun { .. } => false,
                })
                .collect();
            render_plan(&view, &units)
        };

        // Rules are tried in order: a run with the type rule first, a passthrough with the column
        // rule first.
        insta::assert_snapshot!(
            colors_units(&global, vec![type_rule.clone(), column_passthrough.clone()]),
            @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
            "
        );
        insta::assert_snapshot!(
            colors_units(&global, vec![column_passthrough.clone(), type_rule.clone()]),
            @r"
            passthrough #1 only[example.MyPoints:colors]
            "
        );

        // An untyped column matches `Column`, never `Type`.
        let plan_with = |rules: Vec<OwnChunkRule>| {
            let units = plan(&view, &with_rules(global.clone(), rules));
            assert!(plan_covers_view(&view, &units));
            render_plan(&view, &units)
        };
        insta::assert_snapshot!(
            plan_with(vec![OwnChunkRule::new(ColumnSelector::Type(MyPoint::name()))]),
            @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:points]
            merge_split (1048576 B)
              #1 except[example.MyPoints:points]
            merge_split (1048576 B)
              #2 only[example.MyPoints:points]
            merge_split (1048576 B)
              #2 except[example.MyPoints:points]
            merge_split (1048576 B)
              #4 only[example.MyPoints:points]
            merge_split (1048576 B)
              #4 except[example.MyPoints:points]
            merge_split (1048576 B)
              #3 only[example.MyPoints:points]
            merge_split (1048576 B)
              #3 except[example.MyPoints:points]
            "
        );
        insta::assert_snapshot!(
            plan_with(vec![OwnChunkRule::new(ColumnSelector::Column(custom))]),
            @r"
            merge_split (1048576 B)
              #1 all
            merge_split (1048576 B)
              #2 all
            merge_split (1048576 B)
              #4 only[custom]
            merge_split (1048576 B)
              #4 except[custom]
            merge_split (1048576 B)
              #3 all
            "
        );

        // The entity filter: a rule naming `entity` alone leaves `other` mixed; a plain `all()`
        // filter skips the properties subtree; no filter covers every entity, properties included,
        // and still only the columns its selector matches.
        let entity_only = type_rule
            .clone()
            .with_entity_filter(EntityPathFilter::parse_forgiving("+ /entity"));
        insta::assert_snapshot!(plan_with(vec![entity_only]), @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #1 except[example.MyPoints:colors]
            merge_split (1048576 B)
              #2 all
            merge_split (1048576 B)
              #4 all
            merge_split (1048576 B)
              #3 all
            ");

        let plain_all = type_rule
            .clone()
            .with_entity_filter(EntityPathFilter::all());
        insta::assert_snapshot!(plan_with(vec![plain_all]), @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #1 except[example.MyPoints:colors]
            merge_split (1048576 B)
              #2 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #2 except[example.MyPoints:colors]
            merge_split (1048576 B)
              #4 all
            merge_split (1048576 B)
              #3 all
            ");

        assert_eq!(type_rule.entity_filter, None);
        insta::assert_snapshot!(plan_with(vec![type_rule.clone()]), @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #1 except[example.MyPoints:colors]
            merge_split (1048576 B)
              #2 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #2 except[example.MyPoints:colors]
            merge_split (1048576 B)
              #4 all
            merge_split (1048576 B)
              #3 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #3 except[example.MyPoints:colors]
            ");

        // Overrides, under a global target and without one.
        let custom_target = merge_split_settings(64, 0);
        let with_override = |merge_split: MergeSplitOverride| OwnChunkRule {
            merge_split,
            ..type_rule.clone()
        };
        let disabled = OptimizationSettings {
            merge_split: None,
            ..global.clone()
        };

        insta::assert_snapshot!(
            colors_units(&global, vec![with_override(MergeSplitOverride::Inherit)]),
            @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
            "
        );
        insta::assert_snapshot!(
            colors_units(&disabled, vec![with_override(MergeSplitOverride::Inherit)]),
            @r"
            passthrough #1 only[example.MyPoints:colors]
            "
        );
        for settings in [&global, &disabled] {
            insta::allow_duplicates! {
                insta::assert_snapshot!(
                    colors_units(settings, vec![with_override(MergeSplitOverride::Passthrough)]),
                    @r"
                    passthrough #1 only[example.MyPoints:colors]
                    "
                );
                insta::assert_snapshot!(
                    colors_units(
                        settings,
                        vec![with_override(MergeSplitOverride::MergeSplit(custom_target))]
                    ),
                    @r"
                    merge_split (64 B)
                      #1 only[example.MyPoints:colors]
                    "
                );
            }
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
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #1 only[example.MyPoints:colors]
            passthrough #1 except[example.MyPoints:colors]
            passthrough #2 all
            ");

        // An orphan mixed chunk: its `:start` columns are null, so the index records no column
        // for it and it passes through whole.
        let chunks = [
            temporal_mixed_chunk(1, "entity", &[0, 1], 4, 4),
            temporal_mixed_chunk(2, "entity", &[10, 11], 4, 4),
        ];
        let view = view_with_orphan(&chunks);
        let units = plan(&view, &with_rules(settings(1024 * 1024, 0), rules));
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            merge_split (1048576 B)
              #1 only[example.MyPoints:colors]
            merge_split (1048576 B)
              #1 except[example.MyPoints:colors]
            passthrough #2 all
            ");
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

    // --- GoP batching ---

    fn frame() -> Timeline {
        Timeline::new_sequence("frame")
    }

    /// One `VideoStream:sample` per row at the given time points; with `with_keyframe`, every row
    /// also carries an `is_keyframe` cell, `true` on the first row only.
    fn video_rows(
        id: u128,
        entity: &str,
        timepoints: &[Vec<(Timeline, i64)>],
        with_keyframe: bool,
    ) -> Chunk {
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for (i, timepoint) in timepoints.iter().enumerate() {
            let mut batches = vec![
                vec![VideoSample::from(vec![0_u8; 16])]
                    .try_serialized(VideoStream::descriptor_sample())
                    .unwrap(),
            ];
            if with_keyframe {
                batches.push(
                    vec![IsKeyframe::from(i == 0)]
                        .try_serialized(VideoStream::descriptor_is_keyframe())
                        .unwrap(),
                );
            }
            builder = builder.with_serialized_batches(
                RowId::new(),
                timepoint.iter().copied().collect::<TimePoint>(),
                batches,
            );
        }
        builder.build().unwrap()
    }

    /// Sample rows on `frame`.
    fn video_chunk(id: u128, entity: &str, times: &[i64], with_keyframe: bool) -> Chunk {
        let timepoints: Vec<_> = times.iter().map(|&t| vec![(frame(), t)]).collect();
        video_rows(id, entity, &timepoints, with_keyframe)
    }

    /// `is_keyframe` rows only, on `frame`, `true` on the first row only.
    fn keyframe_chunk(id: u128, entity: &str, times: &[i64]) -> Chunk {
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for (i, &time) in times.iter().enumerate() {
            builder = builder.with_serialized_batches(
                RowId::new(),
                [(frame(), time)],
                [vec![IsKeyframe::from(i == 0)]
                    .try_serialized(VideoStream::descriptor_is_keyframe())
                    .unwrap()],
            );
        }
        builder.build().unwrap()
    }

    fn static_codec_chunk(id: u128, entity: &str) -> Chunk {
        Chunk::builder_with_id(ChunkId::from_u128(id), entity)
            .with_serialized_batches(
                RowId::new(),
                TimePoint::default(),
                [vec![VideoCodec::VP8]
                    .try_serialized(VideoStream::descriptor_codec())
                    .unwrap()],
            )
            .build()
            .unwrap()
    }

    /// Sample and codec on every row, on `frame`.
    fn temporal_codec_chunk(id: u128, entity: &str, times: &[i64]) -> Chunk {
        let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
        for &time in times {
            builder = builder.with_serialized_batches(
                RowId::new(),
                [(frame(), time)],
                [
                    vec![VideoSample::from(vec![0_u8; 16])]
                        .try_serialized(VideoStream::descriptor_sample())
                        .unwrap(),
                    vec![VideoCodec::VP8]
                        .try_serialized(VideoStream::descriptor_codec())
                        .unwrap(),
                ],
            );
        }
        builder.build().unwrap()
    }

    fn with_gop_rules(
        mut settings: OptimizationSettings,
        rules: Vec<GopBatchingRule>,
    ) -> OptimizationSettings {
        settings.gop_batching = rules;
        settings
    }

    /// The plan as text, for snapshots: one line per unit, its inputs indented below it, and
    /// chunks named `#<fixture id>`.
    //TODO(ab): consider rolling some custom `Debug` instead (note: derived `Debug` makes overly
    //verbose snapshots).
    fn render_plan(view: &ChunkIndexView, units: &[PlanUnit]) -> String {
        use std::fmt::Write as _;

        let chunk = |idx: ChunkIdx| format!("#{}", view.chunk(idx).chunk_id.as_u128());
        let slice = |slice: &ChunkSlice| {
            let list = |kind: &str, columns: &BTreeSet<ComponentIdentifier>| {
                let columns: Vec<&str> = columns.iter().map(|column| column.as_str()).collect();
                format!("{kind}[{}]", columns.join(", "))
            };
            let columns = match &slice.columns {
                super::ColumnSelection::All => "all".to_owned(),
                super::ColumnSelection::Only(columns) => list("only", columns),
                super::ColumnSelection::Except(columns) => list("except", columns),
            };
            format!("{} {columns}", chunk(slice.chunk))
        };
        let target = |target: &MergeSplitSettings| {
            let MergeSplitSettings {
                max_bytes,
                max_rows,
                max_rows_if_unsorted,
            } = target;
            let mut text = format!("{max_bytes} B");
            if let Some(rows) = max_rows {
                write!(text, ", {rows} rows").ok();
            }
            if let Some(rows) = max_rows_if_unsorted {
                write!(text, ", {rows} unsorted rows").ok();
            }
            text
        };

        let mut lines = Vec::new();
        for unit in units {
            match unit {
                PlanUnit::Passthrough(s) => lines.push(format!("passthrough {}", slice(s))),
                PlanUnit::Drop(s) => lines.push(format!("drop {}", slice(s))),
                PlanUnit::MergeSplitRun {
                    inputs,
                    target: run_target,
                } => {
                    lines.push(format!("merge_split ({})", target(run_target)));
                    lines.extend(inputs.iter().map(|s| format!("  {}", slice(s))));
                }
                PlanUnit::GopRun {
                    entity_path,
                    codec,
                    inputs,
                    timeline,
                    gop_chunking,
                    keyframes,
                } => {
                    let (source, codec_slices) = match codec {
                        CodecSource::Static(slices) => ("static", slices),
                        CodecSource::Temporal(slices) => ("temporal", slices),
                    };
                    let codec_chunks: Vec<String> =
                        codec_slices.iter().map(|s| chunk(s.chunk)).collect();
                    let chunking = match gop_chunking {
                        GopChunkingConfig::OnePerGop => "one per GoP".to_owned(),
                        GopChunkingConfig::Packed(t) => format!("packed ({})", target(t)),
                    };
                    let keyframes = match keyframes {
                        KeyframePlacementConfig::Inline => "inline".to_owned(),
                        KeyframePlacementConfig::OwnChunk(t) => {
                            format!("own chunk ({})", target(t))
                        }
                    };
                    lines.push(format!("gop_run {entity_path} on {timeline}"));
                    lines.push(format!("  codec: {source} {}", codec_chunks.join(" ")));
                    lines.push(format!("  chunking: {chunking}"));
                    lines.push(format!("  keyframes: {keyframes}"));
                    lines.extend(inputs.iter().map(|input| {
                        format!("  {} @{}", slice(&input.slice), input.start.as_i64())
                    }));
                }
            }
        }
        lines.join("\n")
    }

    /// The warnings logged since `log_rx` was registered that name `entity_prefix`, sorted.
    fn warnings_about(
        log_rx: &re_log::Receiver<re_log::LogMsg>,
        entity_prefix: &str,
    ) -> Vec<String> {
        let mut warnings: Vec<String> = log_rx
            .try_iter()
            .map(|msg| msg.message)
            .filter(|message| message.contains(entity_prefix))
            .collect();
        warnings.sort();
        warnings
    }

    fn gop_runs(units: &[PlanUnit]) -> Vec<&PlanUnit> {
        units
            .iter()
            .filter(|unit| matches!(unit, PlanUnit::GopRun { .. }))
            .collect()
    }

    /// A matched entity: the static codec chunk passes through and decides the codec, every
    /// temporal `is_keyframe` cell is dropped, on any timeline set, the samples form one GoP run in
    /// start order, and the sample chunks' other columns go to the entity's rest run.
    #[test]
    fn gop_plan_units() {
        let sample_and_point = Chunk::builder_with_id(ChunkId::from_u128(8), "video")
            .with_serialized_batches(
                RowId::new(),
                [(frame(), 50_i64)],
                [
                    vec![VideoSample::from(vec![0_u8; 16])]
                        .try_serialized(VideoStream::descriptor_sample())
                        .unwrap(),
                    vec![MyPoint::new(1.0, 2.0)]
                        .try_serialized(MyPoints::descriptor_points())
                        .unwrap(),
                ],
            )
            .build()
            .unwrap();
        let chunks = vec![
            static_codec_chunk(1, "video"),
            video_chunk(2, "video", &[0, 1], false),
            video_chunk(3, "video", &[10, 11], true),
            video_chunk(4, "video", &[20, 21], false),
            video_chunk(5, "video", &[30, 31], true),
            keyframe_chunk(6, "video", &[40, 41]),
            Chunk::builder_with_id(ChunkId::from_u128(7), "video")
                .with_serialized_batches(
                    RowId::new(),
                    [(Timeline::new_sequence("other"), 0_i64)],
                    [vec![IsKeyframe::from(true)]
                        .try_serialized(VideoStream::descriptor_is_keyframe())
                        .unwrap()],
                )
                .build()
                .unwrap(),
            sample_and_point,
        ];
        let view = view_of(&chunks);
        let settings = with_gop_rules(settings(1024 * 1024, 0), vec![GopBatchingRule::default()]);

        let units = plan(&view, &settings);

        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #1 all
            drop #3 only[VideoStream:is_keyframe]
            drop #5 only[VideoStream:is_keyframe]
            drop #6 only[VideoStream:is_keyframe]
            drop #7 only[VideoStream:is_keyframe]
            gop_run /video on frame
              codec: static #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #2 only[VideoStream:sample] @0
              #3 only[VideoStream:sample] @10
              #4 only[VideoStream:sample] @20
              #5 only[VideoStream:sample] @30
              #8 only[VideoStream:sample] @50
            merge_split (1048576 B)
              #8 except[VideoStream:sample]
            ");
    }

    /// Rules are tried in order and the first matching filter decides; an unmatched entity's
    /// `is_keyframe` follows the own-chunk rules. Own-chunk rules on `sample` and `codec` are
    /// ignored on a matched entity.
    #[test]
    fn gop_rule_matching() {
        let keyframe_own_rule = OwnChunkRule::new(ColumnSelector::Column(keyframe_column()));
        let global = with_rules(settings(1024 * 1024, 0), vec![keyframe_own_rule.clone()]);

        let chunks = vec![
            static_codec_chunk(1, "a"),
            video_chunk(2, "a", &[0, 1], true),
            static_codec_chunk(3, "b"),
            video_chunk(4, "b", &[0, 1], true),
        ];
        let view = view_of(&chunks);
        let a_passthrough = GopBatchingRule {
            samples: MergeSplitOverride::Passthrough,
            ..GopBatchingRule::default()
        }
        .with_entity_filter(EntityPathFilter::parse_forgiving("+ /a"));
        let plan_with = |rules: Vec<GopBatchingRule>| {
            let units = plan(&view, &with_gop_rules(global.clone(), rules));
            assert!(plan_covers_view(&view, &units));
            render_plan(&view, &units)
        };
        insta::assert_snapshot!(
            plan_with(vec![a_passthrough.clone(), GopBatchingRule::default()]),
            @r"
            passthrough #1 all
            passthrough #3 all
            drop #2 only[VideoStream:is_keyframe]
            gop_run /a on frame
              codec: static #1
              chunking: one per GoP
              keyframes: own chunk (1048576 B)
              #2 only[VideoStream:sample] @0
            drop #4 only[VideoStream:is_keyframe]
            gop_run /b on frame
              codec: static #3
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #4 only[VideoStream:sample] @0
            "
        );
        insta::assert_snapshot!(
            plan_with(vec![GopBatchingRule::default(), a_passthrough.clone()]),
            @r"
            passthrough #1 all
            passthrough #3 all
            drop #2 only[VideoStream:is_keyframe]
            gop_run /a on frame
              codec: static #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #2 only[VideoStream:sample] @0
            drop #4 only[VideoStream:is_keyframe]
            gop_run /b on frame
              codec: static #3
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #4 only[VideoStream:sample] @0
            "
        );
        // `b` is filtered out: its `is_keyframe` follows the own-chunk rule.
        insta::assert_snapshot!(plan_with(vec![a_passthrough]), @r"
            passthrough #1 all
            passthrough #3 all
            drop #2 only[VideoStream:is_keyframe]
            gop_run /a on frame
              codec: static #1
              chunking: one per GoP
              keyframes: own chunk (1048576 B)
              #2 only[VideoStream:sample] @0
            merge_split (1048576 B)
              #4 only[VideoStream:is_keyframe]
            merge_split (1048576 B)
              #4 except[VideoStream:is_keyframe]
            ");

        let chunks = vec![
            temporal_codec_chunk(1, "video", &[0, 1]),
            static_codec_chunk(2, "video"),
        ];
        let view = view_of(&chunks);
        let sample_own_rule = OwnChunkRule::new(ColumnSelector::Column(sample_column()));
        let codec_own_rule = OwnChunkRule::new(ColumnSelector::Column(codec_column()));
        let units = plan(
            &view,
            &with_gop_rules(
                with_rules(global, vec![sample_own_rule, codec_own_rule]),
                vec![GopBatchingRule::default()],
            ),
        );
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #2 all
            drop #1 only[VideoStream:codec]
            gop_run /video on frame
              codec: static #2
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #1 only[VideoStream:sample] @0
            ");
    }

    /// A static codec decides, wherever it is indexed, and shadows every temporal codec cell,
    /// which is dropped. Without one, the codec-only chunks are read, on any timeline set, or else
    /// the first sample chunk carrying the codec, and only the sample chunks' codec cells reach
    /// the run.
    #[test]
    fn gop_codec_source() {
        let codec_only = |id: u128| {
            Chunk::builder_with_id(ChunkId::from_u128(id), "video")
                .with_serialized_batches(
                    RowId::new(),
                    [(Timeline::new_sequence("other"), 0_i64)],
                    [vec![VideoCodec::VP8]
                        .try_serialized(VideoStream::descriptor_codec())
                        .unwrap()],
                )
                .build()
                .unwrap()
        };
        let settings = with_gop_rules(settings(1024 * 1024, 0), vec![GopBatchingRule::default()]);
        let plan_of = |chunks: &[Chunk]| {
            let view = view_of(chunks);
            let units = plan(&view, &settings);
            assert!(plan_covers_view(&view, &units));
            render_plan(&view, &units)
        };

        // Indexed after the temporal codec, the static one still decides.
        insta::assert_snapshot!(
            plan_of(&[
                temporal_codec_chunk(1, "video", &[0, 1]),
                codec_only(2),
                static_codec_chunk(3, "video"),
            ]),
            @r"
            passthrough #3 all
            drop #1 only[VideoStream:codec]
            drop #2 only[VideoStream:codec]
            gop_run /video on frame
              codec: static #3
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #1 only[VideoStream:sample] @0
            "
        );
        insta::assert_snapshot!(
            plan_of(&[
                temporal_codec_chunk(1, "video", &[0, 1]),
                temporal_codec_chunk(2, "video", &[2, 3]),
            ]),
            @r"
            gop_run /video on frame
              codec: temporal #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #1 only[VideoStream:codec, VideoStream:sample] @0
              #2 only[VideoStream:codec, VideoStream:sample] @2
            "
        );
        insta::assert_snapshot!(
            plan_of(&[
                temporal_codec_chunk(1, "video", &[0, 1]),
                codec_only(2),
                codec_only(3),
            ]),
            @r"
            drop #2 only[VideoStream:codec]
            drop #3 only[VideoStream:codec]
            gop_run /video on frame
              codec: temporal #2 #3
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #1 only[VideoStream:codec, VideoStream:sample] @0
            "
        );
    }

    /// A matched entity's static `is_keyframe` is dropped with a warning, so it cannot hide the
    /// regenerated keyframes; the rest of its static chunk passes through.
    #[test]
    fn gop_static_keyframe_dropped() {
        re_log::setup_logging();
        let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::WARN);
        let static_codec_and_keyframe = Chunk::builder_with_id(ChunkId::from_u128(1), "static_kf")
            .with_serialized_batches(
                RowId::new(),
                TimePoint::default(),
                [
                    vec![VideoCodec::VP8]
                        .try_serialized(VideoStream::descriptor_codec())
                        .unwrap(),
                    vec![IsKeyframe::from(true)]
                        .try_serialized(VideoStream::descriptor_is_keyframe())
                        .unwrap(),
                ],
            )
            .build()
            .unwrap();
        let chunks = vec![
            static_codec_and_keyframe,
            video_chunk(2, "static_kf", &[0, 1], false),
        ];
        let view = view_of(&chunks);
        let settings = with_gop_rules(settings(1024 * 1024, 0), vec![GopBatchingRule::default()]);

        let units = plan(&view, &settings);

        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #1 except[VideoStream:is_keyframe]
            drop #1 only[VideoStream:is_keyframe]
            gop_run /static_kf on frame
              codec: static #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #2 only[VideoStream:sample] @0
            ");
        insta::assert_debug_snapshot!(warnings_about(&log_rx, "/static_kf"), @r#"
            [
                "Dropping the static VideoStream:is_keyframe column of a video stream, which would hide the regenerated keyframes\nEntity: /static_kf",
            ]
            "#);
    }

    /// Entities with static samples only are not GoP-batched. Those with temporal samples but no
    /// codec, or with samples on several timeline sets, are skipped with a warning naming them.
    #[test]
    fn gop_unmatched_entities() {
        re_log::setup_logging();
        let log_rx = re_log::add_log_msg_receiver(re_log::LevelFilter::WARN);
        let other = Timeline::new_sequence("other");
        let chunks = vec![
            video_chunk(1, "warned_no_codec", &[0, 1], false),
            static_codec_chunk(2, "warned_two_timeline_sets"),
            video_chunk(3, "warned_two_timeline_sets", &[0, 1], false),
            video_rows(
                4,
                "warned_two_timeline_sets",
                &[vec![(frame(), 2), (other, 0)]],
                false,
            ),
            static_codec_chunk(5, "static_samples"),
            Chunk::builder_with_id(ChunkId::from_u128(6), "static_samples")
                .with_serialized_batches(
                    RowId::new(),
                    TimePoint::default(),
                    [vec![VideoSample::from(vec![0_u8; 16])]
                        .try_serialized(VideoStream::descriptor_sample())
                        .unwrap()],
                )
                .build()
                .unwrap(),
        ];
        let view = view_of(&chunks);
        let settings = with_gop_rules(settings(1024 * 1024, 0), vec![GopBatchingRule::default()]);

        let units = plan(&view, &settings);

        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #2 all
            passthrough #5 all
            passthrough #6 all
            merge_split (1048576 B)
              #1 all
            merge_split (1048576 B)
              #3 all
            merge_split (1048576 B)
              #4 all
            ");
        insta::assert_debug_snapshot!(warnings_about(&log_rx, "/warned_"), @r#"
            [
                "Skipping GoP batching of a video stream whose samples span several timeline sets\nEntity: /warned_two_timeline_sets",
                "Skipping GoP batching of a video stream without a VideoStream:codec column\nEntity: /warned_no_codec",
            ]
            "#);
    }

    /// Sample and keyframe targets resolve against the global target, with and without one.
    #[test]
    fn gop_target_resolution() {
        let chunks = vec![
            static_codec_chunk(1, "video"),
            video_chunk(2, "video", &[0, 1], false),
        ];
        let view = view_of(&chunks);
        let global = settings(1024 * 1024, 0);
        let target = global.merge_split.unwrap();
        let disabled = OptimizationSettings {
            merge_split: None,
            ..global.clone()
        };
        let custom = merge_split_settings(64, 0);
        let resolved = |settings: &OptimizationSettings,
                        samples: MergeSplitOverride,
                        keyframes: KeyframePlacement| {
            let rule = GopBatchingRule {
                entity_filter: None,
                samples,
                keyframes,
            };
            let units = plan(&view, &with_gop_rules(settings.clone(), vec![rule]));
            match gop_runs(&units).as_slice() {
                [
                    PlanUnit::GopRun {
                        gop_chunking,
                        keyframes,
                        ..
                    },
                ] => (*gop_chunking, *keyframes),
                other => panic!("expected one GoP run, got {other:?}"),
            }
        };
        use KeyframePlacement::{Inline, OwnChunk};
        use MergeSplitOverride::{Inherit, MergeSplit, Passthrough};

        assert_eq!(
            resolved(&global, Inherit, OwnChunk(None)),
            (
                GopChunkingConfig::Packed(target),
                KeyframePlacementConfig::OwnChunk(target)
            )
        );
        assert_eq!(
            resolved(&disabled, Inherit, OwnChunk(None)),
            (
                GopChunkingConfig::OnePerGop,
                KeyframePlacementConfig::OwnChunk(super::KEYFRAMES_UNBOUNDED)
            )
        );
        for settings in [&global, &disabled] {
            assert_eq!(
                resolved(settings, Passthrough, OwnChunk(Some(custom))),
                (
                    GopChunkingConfig::OnePerGop,
                    KeyframePlacementConfig::OwnChunk(custom)
                )
            );
            assert_eq!(
                resolved(settings, MergeSplit(custom), Inline),
                (
                    GopChunkingConfig::Packed(custom),
                    KeyframePlacementConfig::Inline
                )
            );
        }
    }

    /// The run timeline is the target timeline when the group has it, else the best by priority:
    /// between user timelines, the last in set order, and any user timeline over `log_time`.
    #[test]
    fn gop_timeline_choice() {
        let other = Timeline::new_sequence("other");
        // Chunk 2 comes first on `frame`, chunk 3 on `other`.
        let chunks = vec![
            static_codec_chunk(1, "video"),
            video_rows(2, "video", &[vec![(frame(), 0), (other, 30)]], false),
            video_rows(3, "video", &[vec![(frame(), 10), (other, 20)]], false),
        ];
        let view = view_of(&chunks);
        let base = with_gop_rules(settings(1024 * 1024, 0), vec![GopBatchingRule::default()]);
        let plan_with_target = |target_timeline: Option<re_log_types::TimelineName>| {
            let settings = OptimizationSettings {
                target_timeline,
                ..base.clone()
            };
            let units = plan(&view, &settings);
            assert!(plan_covers_view(&view, &units));
            render_plan(&view, &units)
        };

        insta::assert_snapshot!(plan_with_target(Some(*other.name())), @r"
            passthrough #1 all
            gop_run /video on other
              codec: static #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #3 only[VideoStream:sample] @20
              #2 only[VideoStream:sample] @30
            ");
        insta::assert_snapshot!(plan_with_target(None), @r"
            passthrough #1 all
            gop_run /video on other
              codec: static #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #3 only[VideoStream:sample] @20
              #2 only[VideoStream:sample] @30
            ");
        insta::assert_snapshot!(plan_with_target(Some(*frame().name())), @r"
            passthrough #1 all
            gop_run /video on frame
              codec: static #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #2 only[VideoStream:sample] @0
              #3 only[VideoStream:sample] @10
            ");

        let chunks = vec![
            static_codec_chunk(1, "video"),
            video_rows(
                2,
                "video",
                &[vec![(frame(), 0), (Timeline::log_time(), 0)]],
                false,
            ),
        ];
        let view = view_of(&chunks);
        let units = plan(&view, &base);
        assert!(plan_covers_view(&view, &units));
        insta::assert_snapshot!(render_plan(&view, &units), @r"
            passthrough #1 all
            gop_run /video on frame
              codec: static #1
              chunking: packed (1048576 B)
              keyframes: own chunk (1048576 B)
              #2 only[VideoStream:sample] @0
            ");
    }
}
