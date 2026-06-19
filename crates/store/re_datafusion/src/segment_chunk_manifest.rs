// Built and unit-tested here; wired into the CPU worker by the next
// PR in the stack. Dead-code lints are silenced module-wide because
// nothing in the production lib path references the API yet.
#![allow(dead_code, clippy::allow_attributes)]

//! Per-segment tracker of which chunks have arrived and which have not.
//!
//! Built from the per-timeline `:start` columns the server attaches to each
//! `QueryDatasetResponse` row (`#1720`). For each segment + entity the
//! manifest holds a multiset of the `time_min` values that the server has
//! committed to delivering. As chunks arrive their `time_min` is decremented
//! out of the multiset, and the [`SegmentChunkManifest::safe_horizon`]
//! advances accordingly:
//!
//! ```text
//! safe_horizon = min(earliest_unreceived_time_min across entities) - 1
//! ```
//!
//! That horizon is the latest timestamp for which every entity contributing
//! to the segment has already produced its data — so rows at or below it
//! are safe to emit under latest-at semantics, and chunks strictly below it
//! are safe to GC out of the segment's in-memory store.
//!
//! The horizon is only meaningful once the worker has seen the *entire*
//! per-segment `chunk_info` list from the server; otherwise a new entity (or
//! a new earlier `time_min`) could appear and retroactively invalidate a
//! previously-emitted horizon. The [`SegmentChunkManifest::lock`] call
//! gates that transition.

use std::collections::{BTreeMap, HashMap};

use re_log_types::{EntityPath, TimeInt};

/// Tracks expected and received per-chunk `time_min` values for a single
/// segment.
///
/// **Lifecycle.**
/// 1. Construct via [`Self::new`].
/// 2. Call [`Self::expect_chunk`] once per `chunk_info` row that targets
///    this segment, supplying the entity path and the chunk's `time_min`
///    on the query's filtered timeline. **Static chunks must be filtered
///    out by the caller** — see [`Self::expect_chunk`] for why.
/// 3. Call [`Self::lock`] once the `chunk_info` list is exhausted. This
///    flips [`Self::is_locked`] and unblocks [`Self::safe_horizon`].
/// 4. As decoded chunks arrive, call [`Self::record_arrival`].
/// 5. Periodically read [`Self::safe_horizon`] and [`Self::is_complete`].
#[derive(Debug, Default)]
pub(crate) struct SegmentChunkManifest {
    /// `entity_path → (time_min → outstanding_count)`. An entry exists
    /// while at least one chunk for that entity at that time is still
    /// unreceived. Empty inner maps are pruned eagerly so
    /// `keys().next()` always points at a real unreceived chunk.
    ///
    /// Keyed by [`EntityPath`] rather than `String` so lookups hash
    /// against the precomputed `EntityPathHash` (u64) and cloning is a
    /// cheap `Arc` bump — consistent with the rest of `re_datafusion`.
    outstanding_time_mins_per_entity: HashMap<EntityPath, BTreeMap<TimeInt, usize>>,

    /// Reverse index of per-entity *head* times. Each key is a
    /// `time_min` that is currently the smallest outstanding `time_min`
    /// for at least one entity; the value is the number of entities
    /// whose head sits at that time.
    ///
    /// Lets [`Self::safe_horizon`] answer in `O(log n)` instead of
    /// scanning every entity's inner map. The first key is the laggard
    /// time across all entities, so the horizon is `first_key - 1` (or
    /// `MAX` when the index is empty). Maintained incrementally by
    /// [`Self::expect_chunk`] and [`Self::record_arrival`]: any change
    /// to an entity's head shifts a single count by 1 in this index.
    ///
    /// This matters because `flush_incremental` calls `safe_horizon`
    /// on every chunk arrival (`ARCHITECTURE.md` §"Per-segment
    /// lifecycle"), and the aggregate per-tick cost scales with the
    /// number of open segments times each manifest's entity count
    /// (`ARCHITECTURE.md` §"Do not drop the segment-count gate", point
    /// 2). Constant-time horizon lookup keeps that aggregate small.
    entity_heads: BTreeMap<TimeInt, usize>,

    /// `true` once the worker has stopped calling [`Self::expect_chunk`].
    /// Until then, [`Self::safe_horizon`] returns `None` even when the
    /// multisets *appear* fully drained, because a not-yet-seen
    /// `chunk_info` row could introduce an earlier `time_min` and
    /// retroactively invalidate any horizon we publish.
    locked: bool,
}

/// Smallest `time_min` currently outstanding for an entity, or `None`
/// if the entity has no outstanding chunks.
fn entity_head(per_entity: &BTreeMap<TimeInt, usize>) -> Option<TimeInt> {
    per_entity.keys().next().copied()
}

/// `entity_heads[t] += 1`.
fn bump_head_index(index: &mut BTreeMap<TimeInt, usize>, t: TimeInt) {
    *index.entry(t).or_insert(0) += 1;
}

/// `entity_heads[t] -= 1`, removing the key when it hits zero.
fn drop_head_index(index: &mut BTreeMap<TimeInt, usize>, t: TimeInt) {
    use std::collections::btree_map::Entry;
    if let Entry::Occupied(mut entry) = index.entry(t) {
        *entry.get_mut() -= 1;
        if *entry.get() == 0 {
            entry.remove();
        }
    }
}

impl SegmentChunkManifest {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Register that a chunk for `entity` with `time_min` is expected to
    /// arrive.
    ///
    /// **Static chunks must not be passed to this method.** Static chunks
    /// carry no temporal `:start` value, and [`TimeInt::STATIC`] sorts
    /// below every temporal value (it is `Self(None)` with a derived
    /// `Ord`). If one slipped into the multiset it would pin
    /// [`Self::safe_horizon`] to a nonsense value forever and the segment
    /// would never make progress. The caller (the `chunk_info` ingest
    /// path) is responsible for filtering static rows out before calling
    /// this method.
    pub(crate) fn expect_chunk(&mut self, entity: EntityPath, time_min: TimeInt) {
        re_log::debug_assert!(
            !self.locked,
            "expect_chunk called after lock(); the manifest is supposed to be append-only \
             before lock and read-only after"
        );
        if time_min.is_static() {
            re_log::debug_assert!(
                false,
                "expect_chunk called with TimeInt::STATIC; callers must pre-filter static chunks \
                 — they have no temporal :start and would corrupt safe_horizon"
            );
            // Release builds must not let a static chunk into the multiset:
            // `TimeInt::STATIC` sorts below every temporal value, so it would
            // pin `safe_horizon` and stall the segment forever. Drop it and
            // surface the upstream bug via the log.
            re_log::error_once!(
                "SegmentChunkManifest::expect_chunk received TimeInt::STATIC; dropping. \
                 This indicates a chunk_info ingest bug — static chunks must be filtered \
                 out before reaching the manifest."
            );
            return;
        }
        let per_entity = self
            .outstanding_time_mins_per_entity
            .entry(entity)
            .or_default();
        let old_head = entity_head(per_entity);
        *per_entity.entry(time_min).or_insert(0) += 1;
        // `expect_chunk` only ever inserts, so the head moves iff the
        // new `time_min` is strictly earlier than the previous head (or
        // the entity had no outstanding chunks yet).
        let head_moved = old_head.is_none_or(|old| time_min < old);
        if head_moved {
            if let Some(old) = old_head {
                drop_head_index(&mut self.entity_heads, old);
            }
            bump_head_index(&mut self.entity_heads, time_min);
        }
    }

    /// Signal that no further [`Self::expect_chunk`] calls will be made
    /// for this segment. Required before [`Self::safe_horizon`] returns
    /// a meaningful value.
    pub(crate) fn lock(&mut self) {
        re_log::debug_assert!(
            !self.locked,
            "lock() called twice on the same manifest; the worker is supposed to lock exactly \
             once after exhausting the per-segment chunk_info list"
        );
        self.locked = true;
    }

    /// Whether [`Self::lock`] has been called.
    pub(crate) fn is_locked(&self) -> bool {
        self.locked
    }

    /// Record the arrival of a chunk for `entity` with `time_min`.
    ///
    /// Returns `true` when the `(entity, time_min)` pair matched an
    /// outstanding expectation and the multiset was decremented; returns
    /// `false` on **manifest/chunk divergence** — either the entity was
    /// never registered, or it was registered but never with this
    /// `time_min`.
    ///
    /// Divergence is silent data loss if not surfaced: the chunk still
    /// inserts into the in-memory store, but because the manifest never
    /// gated the horizon on it, the row range filter
    /// `(processed_through, horizon]` excludes the chunk's rows entirely and
    /// they never emit. See `ARCHITECTURE.md` §"Manifest/chunk
    /// divergence". Callers should `re_log::debug_panic!` +
    /// `re_log::error_once!` on a `false` return; the chunk is still
    /// kept in the store (dropping it would be worse), but the log
    /// surfaces the protocol-level mismatch.
    #[must_use]
    pub(crate) fn record_arrival(&mut self, entity: &EntityPath, time_min: TimeInt) -> bool {
        let Some(per_entity) = self.outstanding_time_mins_per_entity.get_mut(entity) else {
            return false;
        };
        // Capture the head before any mutation; `per_entity` is non-empty
        // here because empty inner maps are pruned on the way out. If the
        // invariant ever breaks, panic in debug to flush it out and
        // degrade to a divergence return in release rather than crashing
        // the worker on a hot path.
        let Some(old_head) = entity_head(per_entity) else {
            re_log::debug_panic!(
                "record_arrival: outer map contains an empty inner map for {entity:?}; \
                 outstanding_time_mins_per_entity invariant violated"
            );
            return false;
        };
        let Some(count) = per_entity.get_mut(&time_min) else {
            // The entity is known but this `time_min` was never expected.
            return false;
        };
        *count -= 1;
        let drained = *count == 0;
        // Release the borrow on `count` before mutating `per_entity` again.
        if drained {
            per_entity.remove(&time_min);
        }
        let new_head = entity_head(per_entity);
        if per_entity.is_empty() {
            self.outstanding_time_mins_per_entity.remove(entity);
        }
        // Head moved iff the smallest outstanding `time_min` for this
        // entity changed. The arrival at `time_min` only moves the head
        // when it drained the current head's slot.
        if Some(old_head) != new_head {
            drop_head_index(&mut self.entity_heads, old_head);
            if let Some(new) = new_head {
                bump_head_index(&mut self.entity_heads, new);
            }
        }
        true
    }

    /// Latest time `T` such that every entity in the segment has either
    /// produced its chunk that contains `T` or has no more chunks at all.
    ///
    /// Returns `None` while the manifest is not yet locked, because a
    /// late-arriving `chunk_info` row could introduce an earlier
    /// `time_min` and invalidate the horizon we'd otherwise publish.
    pub(crate) fn safe_horizon(&self) -> Option<TimeInt> {
        if !self.locked {
            return None;
        }
        let earliest_unreceived = self.entity_heads.keys().next().copied();
        match earliest_unreceived {
            // Every entity has received every announced chunk: nothing
            // gates the horizon — return MAX so callers can drain
            // unconditionally.
            None => Some(TimeInt::MAX),
            // Otherwise the latest *safe* time is one tick before the
            // earliest still-pending chunk's `time_min`. `TimeInt::dec`
            // does the saturating subtract; `MIN - 1` clamps to `MIN`.
            Some(t) => Some(t.dec()),
        }
    }

    /// `true` once [`Self::lock`] has been called *and* every expected
    /// chunk has been recorded as arrived. The fallback / final drain
    /// path in the worker keys off this signal.
    pub(crate) fn is_complete(&self) -> bool {
        // Defensive cross-check: the two maps must co-empty. A drift here
        // would mean `safe_horizon` could still see a head while
        // `is_complete` reports done, which would corrupt the final
        // drain. Cheap O(1) check.
        re_log::debug_assert_eq!(
            self.entity_heads.is_empty(),
            self.outstanding_time_mins_per_entity.is_empty(),
            "entity_heads index drifted from outstanding_time_mins_per_entity"
        );
        self.locked && self.outstanding_time_mins_per_entity.is_empty()
    }

    /// Total number of outstanding expected chunks. Diagnostic / test
    /// helper; production code reads the lifecycle through
    /// [`Self::safe_horizon`] and [`Self::is_complete`].
    #[cfg(test)]
    pub(crate) fn outstanding_count(&self) -> usize {
        self.outstanding_time_mins_per_entity
            .values()
            .map(|per_entity| per_entity.values().copied().sum::<usize>())
            .sum()
    }

    /// Asserts that [`Self::entity_heads`] is consistent with
    /// [`Self::outstanding_time_mins_per_entity`]: for every entity
    /// with outstanding chunks the entity's head appears in the index
    /// once, and the index contains nothing else.
    ///
    /// Call from every mutating test so a regression in the
    /// incremental bookkeeping shows up as a test failure rather than
    /// a silently wrong `safe_horizon`.
    #[cfg(test)]
    fn check_invariants(&self) {
        use std::collections::BTreeMap;
        let mut expected: BTreeMap<TimeInt, usize> = BTreeMap::new();
        for per_entity in self.outstanding_time_mins_per_entity.values() {
            assert!(
                !per_entity.is_empty(),
                "empty inner map should have been pruned"
            );
            let head = entity_head(per_entity).expect("non-empty inner map");
            *expected.entry(head).or_insert(0) += 1;
        }
        assert_eq!(
            self.entity_heads, expected,
            "entity_heads index drifted from outstanding_time_mins_per_entity"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ti(t: i64) -> TimeInt {
        TimeInt::saturated_temporal_i64(t)
    }

    fn ep(path: &str) -> EntityPath {
        EntityPath::from(path)
    }

    /// Pre-lock, the horizon is undefined: a new `expect_chunk` could
    /// arrive with an earlier `time_min` and retroactively shift it.
    #[test]
    fn safe_horizon_is_none_until_locked() {
        let mut m = SegmentChunkManifest::new();
        m.expect_chunk(ep("/a"), ti(10));
        m.expect_chunk(ep("/a"), ti(20));

        assert!(!m.is_locked());
        assert_eq!(m.safe_horizon(), None);
        assert!(!m.is_complete());

        m.lock();
        assert!(m.is_locked());
        // Earliest unreceived is `10` → horizon is `9`.
        assert_eq!(m.safe_horizon(), Some(ti(9)));
        assert!(!m.is_complete());
        m.check_invariants();
    }

    /// `safe_horizon = min(earliest per entity) - 1`. The "lagging"
    /// entity sets the limit.
    #[test]
    fn safe_horizon_is_min_across_entities() {
        let mut m = SegmentChunkManifest::new();
        m.expect_chunk(ep("/a"), ti(100));
        m.expect_chunk(ep("/b"), ti(50));
        m.expect_chunk(ep("/c"), ti(75));
        m.lock();

        // `b` is the laggard at 50 → horizon is 49.
        assert_eq!(m.safe_horizon(), Some(ti(49)));
        m.check_invariants();
    }

    /// Recording arrivals advances the per-entity head. When `/b`'s
    /// chunk lands, `/c` becomes the new laggard.
    #[test]
    fn record_arrival_advances_horizon() {
        let mut m = SegmentChunkManifest::new();
        m.expect_chunk(ep("/a"), ti(100));
        m.expect_chunk(ep("/b"), ti(50));
        m.expect_chunk(ep("/c"), ti(75));
        m.lock();

        assert_eq!(m.safe_horizon(), Some(ti(49)));

        assert!(m.record_arrival(&ep("/b"), ti(50)));
        // `/b` now empty → not part of the min. `/a`=100, `/c`=75 → min 75 → horizon 74.
        assert_eq!(m.safe_horizon(), Some(ti(74)));

        assert!(m.record_arrival(&ep("/c"), ti(75)));
        // `/a` is the only entity left → horizon 99.
        assert_eq!(m.safe_horizon(), Some(ti(99)));

        assert!(m.record_arrival(&ep("/a"), ti(100)));
        // Everyone caught up → horizon is unbounded (MAX).
        assert_eq!(m.safe_horizon(), Some(TimeInt::MAX));
        assert!(m.is_complete());
        m.check_invariants();
    }

    /// Two chunks at the same `time_min` for one entity. The slot has
    /// to survive the first arrival and only drop on the second.
    #[test]
    fn multiset_handles_duplicate_time_min_per_entity() {
        let mut m = SegmentChunkManifest::new();
        m.expect_chunk(ep("/a"), ti(10));
        m.expect_chunk(ep("/a"), ti(10));
        m.lock();

        assert_eq!(m.outstanding_count(), 2);
        assert!(m.record_arrival(&ep("/a"), ti(10)));
        assert_eq!(m.outstanding_count(), 1);
        // Horizon still pinned at 9 because one chunk at time 10 is
        // still expected.
        assert_eq!(m.safe_horizon(), Some(ti(9)));

        assert!(m.record_arrival(&ep("/a"), ti(10)));
        assert_eq!(m.outstanding_count(), 0);
        assert!(m.is_complete());
        assert_eq!(m.safe_horizon(), Some(TimeInt::MAX));
        m.check_invariants();
    }

    /// Out-of-order arrivals: a chunk with a later `time_min` arrives
    /// before the earlier one. The horizon stays pinned at the
    /// still-missing earliest until that one lands.
    #[test]
    fn out_of_order_arrival_keeps_horizon_pinned() {
        let mut m = SegmentChunkManifest::new();
        m.expect_chunk(ep("/a"), ti(10));
        m.expect_chunk(ep("/a"), ti(20));
        m.expect_chunk(ep("/a"), ti(30));
        m.lock();

        // Later chunk arrives first.
        assert!(m.record_arrival(&ep("/a"), ti(30)));
        assert_eq!(m.safe_horizon(), Some(ti(9)));

        assert!(m.record_arrival(&ep("/a"), ti(20)));
        assert_eq!(m.safe_horizon(), Some(ti(9)));

        assert!(m.record_arrival(&ep("/a"), ti(10)));
        assert_eq!(m.safe_horizon(), Some(TimeInt::MAX));
        assert!(m.is_complete());
        m.check_invariants();
    }

    /// Manifest/chunk divergence: an arrival that doesn't match any
    /// outstanding expectation must return `false` so the worker can
    /// `debug_panic!` + `error_once!`. Both divergence shapes — unknown
    /// entity and known entity with unexpected `time_min` — return
    /// `false`; the legitimate arrival returns `true`. The multiset is
    /// left untouched by the divergent calls so we don't corrupt
    /// horizon math by reacting to a malformed server response.
    /// See `ARCHITECTURE.md` §"Manifest/chunk divergence".
    #[test]
    fn unexpected_arrival_returns_false() {
        let mut m = SegmentChunkManifest::new();
        m.expect_chunk(ep("/a"), ti(10));
        m.lock();

        // Unknown entity.
        assert!(!m.record_arrival(&ep("/b"), ti(10)));
        // Known entity, unexpected `time_min`.
        assert!(!m.record_arrival(&ep("/a"), ti(999)));

        // Divergent calls must not have decremented the multiset.
        assert_eq!(m.outstanding_count(), 1);
        assert_eq!(m.safe_horizon(), Some(ti(9)));

        // The legitimate arrival returns `true` and drains the slot.
        assert!(m.record_arrival(&ep("/a"), ti(10)));
        assert_eq!(m.outstanding_count(), 0);
        assert!(m.is_complete());
        m.check_invariants();
    }

    /// Locked + zero expectations: horizon is fully open immediately,
    /// the segment is trivially complete. Covers the "static-only
    /// query" and "empty segment" fallback paths.
    #[test]
    fn empty_segment_after_lock_is_complete_with_max_horizon() {
        let mut m = SegmentChunkManifest::new();
        m.lock();

        assert!(m.is_complete());
        assert_eq!(m.safe_horizon(), Some(TimeInt::MAX));
        m.check_invariants();
    }

    /// Saturating subtraction protects against `TimeInt::MIN`
    /// underflow if the server ever ships a chunk with `time_min == MIN`.
    #[test]
    fn horizon_saturates_at_time_int_min() {
        let mut m = SegmentChunkManifest::new();
        m.expect_chunk(ep("/a"), TimeInt::MIN);
        m.lock();

        let h = m.safe_horizon().expect("locked manifest has a horizon");
        // `MIN - 1` saturates back to `MIN` rather than wrapping.
        assert_eq!(h, TimeInt::MIN);
        m.check_invariants();
    }

    /// Stress test for the incremental `entity_heads` bookkeeping:
    /// interleave expects and arrivals across multiple entities so the
    /// head index has to handle every transition shape (new head,
    /// head holds, head advances, entity drains, entity comes back via
    /// later expect — though after `lock` the latter cannot happen).
    /// `check_invariants` runs after each step.
    #[test]
    fn entity_heads_stay_in_sync_under_mixed_ops() {
        let mut m = SegmentChunkManifest::new();
        // Pre-lock interleaving.
        m.expect_chunk(ep("/a"), ti(10));
        m.check_invariants();
        m.expect_chunk(ep("/b"), ti(30));
        m.check_invariants();
        m.expect_chunk(ep("/a"), ti(20)); // /a head stays at 10
        m.check_invariants();
        m.expect_chunk(ep("/b"), ti(5)); // /b head moves from 30 to 5
        m.check_invariants();
        m.expect_chunk(ep("/c"), ti(15));
        m.check_invariants();
        m.expect_chunk(ep("/a"), ti(10)); // duplicate; /a head stays at 10
        m.check_invariants();

        m.lock();
        // Heads: /a=10, /b=5, /c=15 → laggard 5 → horizon 4.
        assert_eq!(m.safe_horizon(), Some(ti(4)));
        m.check_invariants();

        // Drain /b's only head; /b empties out, laggard becomes /a@10.
        assert!(m.record_arrival(&ep("/b"), ti(5)));
        m.check_invariants();
        assert!(m.record_arrival(&ep("/b"), ti(30)));
        m.check_invariants();
        assert_eq!(m.safe_horizon(), Some(ti(9)));

        // Duplicate at /a@10 still pending; first arrival doesn't move head.
        assert!(m.record_arrival(&ep("/a"), ti(10)));
        m.check_invariants();
        assert_eq!(m.safe_horizon(), Some(ti(9)));

        // Second /a@10 arrival drains the slot; /a head advances to 20.
        // Laggard now /c@15 → horizon 14.
        assert!(m.record_arrival(&ep("/a"), ti(10)));
        m.check_invariants();
        assert_eq!(m.safe_horizon(), Some(ti(14)));

        // Drain /c, then /a@20. Horizon goes to MAX.
        assert!(m.record_arrival(&ep("/c"), ti(15)));
        m.check_invariants();
        assert!(m.record_arrival(&ep("/a"), ti(20)));
        m.check_invariants();

        assert!(m.is_complete());
        assert_eq!(m.safe_horizon(), Some(TimeInt::MAX));
    }
}
