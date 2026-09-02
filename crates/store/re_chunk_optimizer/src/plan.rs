//! The planner: a pure function from a [`ChunkIndexView`] and [`OptimizationSettings`] to a list
//! of plan nodes.

use re_log_types::TimelineName;

use crate::settings::{MergeSplitSettings, OptimizationSettings};
use crate::view::{ChunkIdx, ChunkIndexView, TimelineSetGroup};

/// One piece of the plan.
//TODO(ab): add more planning primitives (GoP, own chunk, etc.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanUnit {
    /// Pass the chunk unmodified.
    Passthrough(ChunkIdx),

    /// Load the provided chunks in order, merging/splitting them on the way per the target
    /// settings.
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
        inputs: Vec<ChunkIdx>,
        target: MergeSplitSettings,
    },
}

/// Build a plan.
pub fn plan(view: &ChunkIndexView, settings: OptimizationSettings) -> Vec<PlanUnit> {
    let mut units = Vec::new();
    let mut claimed = vec![false; view.num_chunks()];

    // Static chunks pass through, in chunk-index order.
    for (idx, meta) in view.chunks() {
        if meta.is_static {
            claimed[idx.as_usize()] = true;
            units.push(PlanUnit::Passthrough(idx));
        }
    }

    // Temporal chunks merge per entity and per exact timeline set — the same grouping the merge
    // gate `Chunk::concatenable` enforces at merge time.
    for entity in view.entities.values() {
        for group in &entity.timeline_sets {
            let order = sweep_order(view, group, settings.target_timeline.as_ref());
            for &idx in &order {
                claimed[idx.as_usize()] = true;
            }
            match settings.merge_split {
                Some(target) => units.push(PlanUnit::MergeSplitRun {
                    inputs: order,
                    target,
                }),
                None => units.extend(order.into_iter().map(PlanUnit::Passthrough)),
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
            units.push(PlanUnit::Passthrough(idx));
        }
    }

    re_log::debug_assert!(
        plan_covers_view(view, &units),
        "every chunk must land in exactly one node"
    );

    units
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

/// Check the plan invariant: every `ChunkIdx` of the view appears in exactly one unit.
pub fn plan_covers_view(view: &ChunkIndexView, outputs: &[PlanUnit]) -> bool {
    let mut seen = vec![false; view.num_chunks()];

    let mut claim = |idx: ChunkIdx| {
        let slot = &mut seen[idx.as_usize()];
        let fresh = !*slot;
        *slot = true;
        fresh
    };

    for output in outputs {
        match output {
            PlanUnit::Passthrough(idx) => {
                if !claim(*idx) {
                    return false;
                }
            }

            PlanUnit::MergeSplitRun { inputs, target: _ } => {
                if inputs.is_empty() || !inputs.iter().all(|&idx| claim(idx)) {
                    return false;
                }
            }
        }
    }

    seen.into_iter().all(|claimed| claimed)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use re_chunk::{Chunk, ChunkId, RowId};
    use re_log_encoding::RawRrdManifest;
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{StoreId, StoreKind, Timeline};
    use re_types_core::ComponentBatch as _;

    use super::{PlanUnit, plan, plan_covers_view};
    use crate::view::{ChunkIdx, ChunkIndexView};
    use crate::{MergeSplitSettings, OptimizationSettings};

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

    fn settings(max_bytes: u64, max_rows: u64) -> OptimizationSettings {
        OptimizationSettings {
            merge_split: Some(MergeSplitSettings {
                max_bytes: NonZeroU64::new(max_bytes).unwrap(),
                max_rows: NonZeroU64::new(max_rows),
                max_rows_if_unsorted: None,
            }),
            target_timeline: None,
        }
    }

    fn merge_split_run(
        view: &ChunkIndexView,
        ids: &[u128],
        settings: &OptimizationSettings,
    ) -> PlanUnit {
        PlanUnit::MergeSplitRun {
            inputs: ids.iter().map(|&id| idx_of(view, id)).collect(),
            target: settings.merge_split.unwrap(),
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
        let outputs = plan(&view, settings);
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
        let outputs = plan(&view, settings);
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
        let outputs = plan(&view, settings(1, 0));
        assert!(plan_covers_view(&view, &outputs));
        assert!(matches!(
            outputs[0],
            PlanUnit::Passthrough(idx) if idx == idx_of(&view, 1)
        ));
        assert!(matches!(&outputs[1], PlanUnit::MergeSplitRun { .. }));

        // `merge_split: None`: every chunk stands alone, no run.
        let disabled = OptimizationSettings {
            merge_split: None,
            target_timeline: None,
        };
        let outputs = plan(&view, disabled);
        assert!(plan_covers_view(&view, &outputs));
        assert_eq!(
            outputs,
            vec![
                PlanUnit::Passthrough(idx_of(&view, 1)),
                PlanUnit::Passthrough(idx_of(&view, 2)),
                PlanUnit::Passthrough(idx_of(&view, 3)),
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
        };

        // Time order sweeps 0, 10, 20, 30.
        let outputs = plan(&view, with_target("frame"));
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
        assert_eq!(plan(&view, settings(1024 * 1024, 0)), file_order);
        assert_eq!(
            plan(&view, with_target("no_such_timeline")),
            vec![merge_split_run(
                &view,
                &[1, 2, 3, 4],
                &with_target("no_such_timeline")
            )]
        );
    }

    /// A non-static chunk whose time range is null on every (index, component) pair is invisible
    /// to the temporal map, so it lands in no group — the plan must still pass it through instead
    /// of silently dropping its rows.
    #[test]
    fn orphan_chunk_passes_through() {
        use arrow::array::BooleanArray;

        let chunks = [
            temporal_chunk(1, "entity", &[0, 1], 64),
            temporal_chunk(2, "entity", &[10, 11], 64),
        ];
        let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
        let mut raw = RawRrdManifest::build_in_memory_from_chunks(store_id, chunks.iter()).unwrap();

        // Null out the second chunk's start/end on every (index, component) pair column. Rows
        // follow append order, so row 1 is chunk 2.
        let orphan_mask = BooleanArray::from(vec![false, true]);
        let schema = raw.data.schema();
        let columns = std::iter::zip(schema.fields(), raw.data.columns())
            .map(|(field, column)| {
                // The same identification `calc_temporal_map` uses for the per-(index, component)
                // time-range pair columns.
                let is_pair_range = field.metadata().contains_key("rerun:index")
                    && field.metadata().contains_key("rerun:component")
                    && (field.name().ends_with(":start") || field.name().ends_with(":end"));
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

        let view = ChunkIndexView::try_from_raw(&raw).unwrap();
        let settings = settings(1024 * 1024, 0);
        let units = plan(&view, settings);

        assert!(plan_covers_view(&view, &units));
        assert_eq!(
            units,
            vec![
                merge_split_run(&view, &[1], &settings),
                PlanUnit::Passthrough(idx_of(&view, 2)),
            ]
        );
    }
}
