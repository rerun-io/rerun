//! Per-segment state: the plan cursor (watermark) plus the [`SegmentStore`]
//! and the segment's issuance-window ledger.
//!
//! Because the plan is immutable shared data available before anything runs,
//! each segment's safe horizon is a pure function of which planned chunks
//! have been delivered: a chunk can never outrun the metadata describing it.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;

use re_dataframe::QueryExpression;
use re_dataframe::external::re_chunk::ChunkId;
use re_log_types::TimeInt;

use super::plan::{CursorKey, PlannedSegment, SegmentEmitMode};
use super::window::SegmentLedger;
use crate::dataframe_query_common::IndexValuesMap;
use crate::dataframe_query_provider::segment_store::SegmentStore;

/// Watermark over the planned chunk multiset of one segment.
///
/// Seeded from [`PlannedSegment::cursor_keys`]; each delivered chunk
/// removes its key. The safe horizon is derived from the earliest key still
/// outstanding. Keyed by [`CursorKey`] rather than by plan position, so the
/// watermark holds under out-of-order delivery.
pub(crate) struct PlanCursor {
    /// Undelivered planned chunks, as key → count. Counts are always
    /// non-zero: a key is removed outright on its last delivery, which is
    /// what lets emptiness stand in for completion.
    outstanding: BTreeMap<CursorKey, u32>,
}

impl PlanCursor {
    pub fn new(outstanding: BTreeMap<CursorKey, u32>) -> Self {
        re_log::debug_assert!(
            outstanding.values().all(|&count| count > 0),
            "plan cursor seeded with a zero-count key",
        );
        Self { outstanding }
    }

    /// Record delivery of a planned chunk with the given key.
    ///
    /// Divergence (a key that was never planned, or delivered more times
    /// than planned) fires `debug_panic!` — the router matches deliveries
    /// by exact `ChunkId` against the plan before calling this, so a miss
    /// here indicates a bookkeeping bug, not a server anomaly.
    pub fn mark_delivered(&mut self, key: CursorKey) {
        match self.outstanding.entry(key) {
            Entry::Occupied(mut entry) if *entry.get() > 1 => *entry.get_mut() -= 1,
            Entry::Occupied(entry) => {
                entry.remove();
            }
            Entry::Vacant(_) => {
                re_log::debug_panic!(
                    "plan-cursor divergence: delivered chunk key {key:?} was not outstanding"
                );
            }
        }
    }

    /// Every planned chunk has been delivered.
    pub fn is_complete(&self) -> bool {
        self.outstanding.is_empty()
    }

    /// The safe horizon: rows at `T <= horizon` may emit.
    ///
    /// * `None` — nothing is emittable yet, for either of two reasons. A
    ///   `PreTime` chunk (static / null-`:start`) is outstanding, so any row
    ///   could still be affected by data with no position on the timeline.
    ///   Or the earliest outstanding chunk sits at [`TimeInt::MIN`], below
    ///   which there is no representable tick to emit up to.
    /// * `Some(t)` — every chunk with `time_min <= t` has been delivered.
    ///   `t` is the earliest outstanding `time_min`, minus one. It is always
    ///   *strictly* below every outstanding key, which is the property the
    ///   emit range `(processed, horizon]` relies on.
    /// * Complete segments report `Some(TimeInt::MAX)`; callers typically
    ///   switch to the final drain instead.
    pub fn safe_horizon(&self) -> Option<TimeInt> {
        match self.outstanding.first_key_value() {
            None => Some(TimeInt::MAX),
            Some((CursorKey::PreTime, _)) => None,

            // `TimeInt::MIN.dec()` saturates back to `MIN`, so reporting a
            // horizon here would include the outstanding chunk's own rows.
            // The emit range is inclusive of the horizon and `processed`
            // then advances past `MIN`, so those rows would never be
            // queried again — silent loss rather than late emission.
            Some((CursorKey::Time(t), _)) if *t == TimeInt::MIN => None,

            Some((CursorKey::Time(t), _)) => Some(t.dec()),
        }
    }
}

/// Everything the driver tracks for one in-flight segment.
pub(crate) struct SegmentState {
    /// The store/emit/GC core.
    pub store: SegmentStore,
    pub cursor: PlanCursor,

    /// Window bytes held on behalf of this segment's delivered,
    /// not-yet-GC'd chunks. Dropping the state refunds the residual.
    pub ledger: SegmentLedger,

    /// This segment's slice of the plan, shared with it rather than copied:
    /// the source for delivery routing (`chunk_meta`) and the emit mode.
    pub planned: Arc<PlannedSegment>,
}

impl SegmentState {
    /// Create the state for a segment. Called lazily, on the segment's
    /// first delivered chunk.
    pub fn new(
        origin: re_uri::Origin,
        planned: Arc<PlannedSegment>,
        base_query_expression: &QueryExpression,
        index_values: &IndexValuesMap,
    ) -> Self {
        Self {
            store: SegmentStore::new(
                origin,
                planned.segment_id.clone(),
                base_query_expression,
                index_values,
            ),
            cursor: PlanCursor::new(planned.cursor_keys()),
            ledger: SegmentLedger::default(),
            planned,
        }
    }

    pub fn mode(&self) -> SegmentEmitMode {
        self.planned.mode
    }

    /// Plan-estimate bytes for the chunks a GC pass removed, resolved via
    /// the plan's `chunk_meta` (single-currency rule: releases are in
    /// estimate space, never measured store bytes).
    ///
    /// A `chunk_id` absent from `chunk_meta` is a plan divergence — the GC
    /// pass can only remove chunks the store holds, and the store only ever
    /// holds chunks this plan itself fetched — so it fires `debug_panic!`
    /// rather than silently undercounting, matching
    /// [`PlanCursor::mark_delivered`].
    pub fn est_bytes_for(&self, chunk_ids: impl IntoIterator<Item = ChunkId>) -> u64 {
        chunk_ids
            .into_iter()
            .map(|id| {
                if let Some(&(_key, est_bytes)) = self.planned.chunk_meta.get(&id) {
                    est_bytes
                } else {
                    re_log::debug_panic!(
                        "plan-cursor divergence: released chunk {id} was not planned"
                    );
                    0
                }
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: i64) -> CursorKey {
        CursorKey::Time(TimeInt::new_temporal(v))
    }

    fn cursor(keys: &[(CursorKey, u32)]) -> PlanCursor {
        PlanCursor::new(keys.iter().copied().collect())
    }

    /// The horizon is the earliest outstanding `time_min` minus one, and
    /// advances only as those chunks are delivered.
    #[test]
    fn horizon_is_earliest_outstanding_minus_one() {
        let mut c = cursor(&[(t(10), 1), (t(20), 1), (t(30), 1)]);
        assert_eq!(c.safe_horizon(), Some(TimeInt::new_temporal(9)));

        c.mark_delivered(t(10));
        assert_eq!(c.safe_horizon(), Some(TimeInt::new_temporal(19)));

        c.mark_delivered(t(20));
        assert_eq!(c.safe_horizon(), Some(TimeInt::new_temporal(29)));

        c.mark_delivered(t(30));
        assert!(c.is_complete());
        assert_eq!(c.safe_horizon(), Some(TimeInt::MAX));
    }

    /// Delivering a later chunk first must not advance the horizon past the
    /// earliest outstanding one. This is the case the multiset exists for —
    /// it holds under `buffer_unordered` delivery.
    #[test]
    fn out_of_order_delivery_keeps_horizon_pinned() {
        let mut c = cursor(&[(t(10), 1), (t(20), 1)]);

        c.mark_delivered(t(20));
        assert_eq!(
            c.safe_horizon(),
            Some(TimeInt::new_temporal(9)),
            "horizon stays pinned at the earliest outstanding time_min - 1",
        );

        c.mark_delivered(t(10));
        assert_eq!(c.safe_horizon(), Some(TimeInt::MAX));
    }

    /// Two planned chunks with the same `time_min` must both gate the
    /// horizon.
    #[test]
    fn duplicate_time_min_gates_until_both_delivered() {
        let mut c = cursor(&[(t(10), 2), (t(20), 1)]);

        c.mark_delivered(t(10));
        assert_eq!(
            c.safe_horizon(),
            Some(TimeInt::new_temporal(9)),
            "one of two chunks at t=10 delivered; horizon must not advance",
        );

        c.mark_delivered(t(10));
        assert_eq!(c.safe_horizon(), Some(TimeInt::new_temporal(19)));
    }

    /// `TimeInt::MIN` is the floor of the temporal range, so there is no
    /// tick below an outstanding chunk sitting there. Reporting `MIN` as the
    /// horizon would emit that chunk's own rows and then advance `processed`
    /// past `MIN`, dropping them for good.
    #[test]
    fn outstanding_at_time_int_min_gates_all_emission() {
        let mut c = cursor(&[(CursorKey::Time(TimeInt::MIN), 1), (t(10), 1)]);
        assert_eq!(c.safe_horizon(), None);

        c.mark_delivered(CursorKey::Time(TimeInt::MIN));
        assert_eq!(c.safe_horizon(), Some(TimeInt::new_temporal(9)));
    }

    /// The horizon must sit *strictly* below every outstanding key — the
    /// emit range is `(processed, horizon]`, so a horizon equal to an
    /// outstanding `time_min` ships rows that chunk still contributes to.
    #[test]
    fn horizon_is_strictly_below_every_outstanding_key() {
        for keys in [
            vec![(CursorKey::Time(TimeInt::MIN), 1)],
            vec![(CursorKey::Time(TimeInt::MIN), 1), (t(10), 1)],
            vec![(t(TimeInt::MIN.as_i64() + 1), 1)],
            vec![(t(0), 1)],
            vec![(t(10), 2), (t(20), 1)],
        ] {
            let c = cursor(&keys);
            if let Some(horizon) = c.safe_horizon() {
                let earliest = keys[0].0;
                assert!(
                    CursorKey::Time(horizon) < earliest,
                    "horizon {horizon:?} must be strictly below outstanding {earliest:?}",
                );
            }
        }
    }

    /// A segment with no planned chunks is born complete with horizon MAX.
    #[test]
    fn empty_plan_is_complete_with_max_horizon() {
        let c = cursor(&[]);
        assert!(c.is_complete());
        assert_eq!(c.safe_horizon(), Some(TimeInt::MAX));
    }

    /// While a `PreTime` chunk (static / null-`:start`) is outstanding,
    /// nothing may emit: it could carry values affecting any row.
    #[test]
    fn pretime_outstanding_gates_all_emission() {
        let mut c = cursor(&[(CursorKey::PreTime, 1), (t(10), 1)]);
        assert_eq!(c.safe_horizon(), None, "PreTime outstanding gates emission");

        c.mark_delivered(CursorKey::PreTime);
        assert_eq!(c.safe_horizon(), Some(TimeInt::new_temporal(9)));
    }

    /// Divergent delivery (a key never planned) must leave the outstanding
    /// set untouched.
    #[test]
    #[cfg(not(debug_assertions))] // debug builds debug_panic! here by design
    fn divergent_delivery_is_ignored() {
        let mut c = cursor(&[(t(10), 1)]);
        c.mark_delivered(t(99));
        assert!(!c.is_complete());
        assert_eq!(c.safe_horizon(), Some(TimeInt::new_temporal(9)));
    }

    #[test]
    #[should_panic(expected = "plan-cursor divergence")]
    #[cfg(debug_assertions)]
    fn divergent_delivery_debug_panics() {
        let mut c = cursor(&[(t(10), 1)]);
        c.mark_delivered(t(99));
    }
}
