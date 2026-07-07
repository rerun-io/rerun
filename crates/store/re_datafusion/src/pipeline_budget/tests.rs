use std::sync::Arc;

use super::*;

// Default constants are tuned to leave the budget effectively
// disengaged (FRACTION=1.0, MIN=4 GiB, MAX=1 TiB). The tests below
// assert the clamp logic still selects the right bound at the
// extremes — not that the budget meaningfully restricts in-flight
// bytes at typical data sizes. Once the CPU-worker streaming-release
// refactor lands and the constants come back down (FRACTION=0.25,
// MIN=64 MiB, MAX=1 GiB), tighten these to also assert proportional
// sizing in the small/medium ranges.

#[test]
fn test_budget_clamps_to_min() {
    // 10 MB total, 1 partition → 100% = 10 MB, clamped to MIN_BUDGET_PER_PARTITION (4 GiB)
    let budget = PipelineBudget::new(10 * 1024 * 1024, 1);
    assert_eq!(budget.budget, MIN_BUDGET_PER_PARTITION);
}

#[test]
fn test_budget_clamps_to_max() {
    // 8 PiB total, 1 partition → 100% = 8 PiB, clamped to MAX_BUDGET_PER_PARTITION (1 TiB)
    let budget = PipelineBudget::new(8 * 1024 * 1024 * 1024 * 1024 * 1024, 1);
    assert_eq!(budget.budget, MAX_BUDGET_PER_PARTITION);
}

#[test]
fn test_budget_scales_with_partitions() {
    // 64 PiB total, 14 partitions → 100% / 14 ≈ 4.6 PiB per partition,
    // clamped to MAX_BUDGET_PER_PARTITION (1 TiB) each.
    let budget = PipelineBudget::new(64 * 1024 * 1024 * 1024 * 1024 * 1024, 14);
    assert_eq!(budget.budget, MAX_BUDGET_PER_PARTITION * 14);
}

#[test]
fn test_budget_small_data_many_partitions() {
    // 100 MB total, 4 partitions → 100% / 4 = 25 MB per partition,
    // clamped to MIN_BUDGET_PER_PARTITION (4 GiB) each.
    let budget = PipelineBudget::new(100 * 1024 * 1024, 4);
    assert_eq!(budget.budget, MIN_BUDGET_PER_PARTITION * 4);
}

#[tokio::test]
async fn test_reserve_blocks_when_budget_exhausted() {
    let budget = Arc::new(PipelineBudget::new(0, 1)); // clamps up to MIN_BUDGET_PER_PARTITION (4 GiB)
    budget.set_multiplier(1.0);
    let half = budget.budget / 2;

    // First reserve should succeed immediately
    budget.reserve(half).await;
    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        half
    );

    // Second reserve should also succeed (half + half = budget)
    budget.reserve(half).await;
    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        half * 2
    );

    // Third reserve should block because budget is full.
    let budget_clone = Arc::clone(&budget);
    let handle = tokio::spawn(async move {
        budget_clone.reserve(half).await;
    });

    // Give the task a chance to run (it should be blocked)
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(
        !handle.is_finished(),
        "reserve should block when budget is exhausted"
    );

    // Release enough to unblock
    budget.release(half);

    // Now the spawned task should complete
    tokio::time::timeout(std::time::Duration::from_secs(1), handle)
        .await
        .expect("reserve should unblock after release")
        .expect("task should not panic");
}

#[tokio::test]
async fn test_adjust_reservation_corrects_estimate() {
    let budget = Arc::new(PipelineBudget::new(0, 1)); // clamps up to MIN_BUDGET_PER_PARTITION (4 GiB)
    budget.set_multiplier(1.0);

    let estimated = 1000;
    let actual = 600;

    let reserved = budget.reserve(estimated).await;
    assert_eq!(reserved, estimated);
    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        estimated
    );

    budget.adjust_reservation(estimated, reserved, actual);
    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        actual
    );
}

#[tokio::test]
async fn test_estimate_multiplier_adapts_over_time() {
    // With the 1.5x bootstrap, a task estimating 1000 bytes initially
    // reserves 1500. As we feed back samples showing actual is 2x the
    // raw estimate, the learned multiplier climbs toward 2.0 and
    // subsequent reserves size accordingly.
    let budget = Arc::new(PipelineBudget::new(0, 1));
    assert_eq!(budget.current_multiplier(), INITIAL_ESTIMATE_MULTIPLIER);

    let estimated = 1000;
    let actual = 2000; // true ratio = 2.0

    // First sample: reserve under bootstrap, then teach the EMA.
    let reserved1 = budget.reserve(estimated).await;
    assert_eq!(reserved1, 1500);
    budget.adjust_reservation(estimated, reserved1, actual);
    budget.release(actual);

    // Multiplier has nudged toward 2.0 but isn't there yet
    // (EMA: 0.2 * 2.0 + 0.8 * 1.5 = 1.6).
    assert!((budget.current_multiplier() - 1.6).abs() < 1e-9);

    // After many samples at ratio=2.0 the multiplier converges.
    for _ in 0..40 {
        let reserved = budget.reserve(estimated).await;
        budget.adjust_reservation(estimated, reserved, actual);
        budget.release(actual);
    }
    assert!(
        (budget.current_multiplier() - 2.0).abs() < 0.01,
        "multiplier should converge toward 2.0, got {}",
        budget.current_multiplier()
    );
}

#[tokio::test]
async fn test_estimate_multiplier_is_clamped() {
    // A pathological 10x expansion should not blow out the multiplier;
    // it saturates at MAX_ESTIMATE_MULTIPLIER. Raw samples are clamped
    // before entering the EMA, so the EMA itself converges toward the
    // clamped value (3.0) rather than the raw ratio (10.0).
    let budget = Arc::new(PipelineBudget::new(0, 1));
    for _ in 0..100 {
        budget.record_actual_sample(100, 1000); // raw ratio = 10
    }
    assert!(
        (budget.current_multiplier() - MAX_ESTIMATE_MULTIPLIER).abs() < 1e-4,
        "multiplier should converge to MAX, got {}",
        budget.current_multiplier()
    );

    // And undersized actual (ratio < 1) floors at MIN_ESTIMATE_MULTIPLIER.
    for _ in 0..100 {
        budget.record_actual_sample(1000, 100); // raw ratio = 0.1
    }
    assert!(
        (budget.current_multiplier() - MIN_ESTIMATE_MULTIPLIER).abs() < 1e-4,
        "multiplier should converge to MIN, got {}",
        budget.current_multiplier()
    );
}

#[tokio::test]
async fn test_peak_current_tracks_high_water_mark() {
    let budget = Arc::new(PipelineBudget::new(0, 1));
    budget.set_multiplier(1.0);

    // Reserve, release, reserve smaller — peak should reflect the
    // larger of the two in-flight values, not the latest.
    let r1 = budget.reserve(10 * 1024 * 1024).await;
    let r2 = budget.reserve(5 * 1024 * 1024).await;
    let peak_before = budget
        .peak_current
        .load(std::sync::atomic::Ordering::Acquire);
    assert_eq!(peak_before, r1 + r2);

    budget.release(r1 + r2);
    let r3 = budget.reserve(1024 * 1024).await;
    let peak_after = budget
        .peak_current
        .load(std::sync::atomic::Ordering::Acquire);
    assert_eq!(
        peak_after, peak_before,
        "peak should not regress after releases",
    );

    budget.release(r3);
}

// Stress test for the CAS-based fast path: launches many concurrent
// reservers whose combined ask fits exactly into the budget. Under
// the previous `fetch_add → check → fetch_sub` design these would
// spuriously inflate `current` past the cap and bounce off each
// other. With CAS, only the actually-committed reservations move
// `current`, so all tasks complete on the fast path with `current`
// landing exactly at `budget`.
#[tokio::test]
async fn test_reserve_no_thundering_herd_under_contention() {
    let budget = Arc::new(PipelineBudget::new(0, 1));
    budget.set_multiplier(1.0);
    let full = budget.budget;
    let n: usize = 32;
    let per_task = full / n;
    assert!(per_task > 0, "budget too small for {n}-way contention test");

    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let budget = Arc::clone(&budget);
        handles.push(tokio::spawn(async move { budget.reserve(per_task).await }));
    }

    for handle in handles {
        tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("reserve hung under contention")
            .expect("task panicked");
    }

    // All `n` reservations should have committed exactly once each.
    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        per_task * n,
    );
}

// Regression test for the lost-wakeup race in `reserve`'s wait
// path:
//
//   reserve():  try_acquire    -> fail (over budget)
//               wait_queue.push_back(notify)
//                   ↓  ← if release runs here, must either be seen
//                        on the second try_acquire below OR pop the
//                        notify we just enqueued. Either path keeps
//                        the reserver from awaiting forever.
//               try_acquire    -> retry / await
//               notify.notified().await
//
// The test deterministically opens the post-enqueue window with
// the pause hook, fires a `release` while the reserver is parked,
// and asserts the reserver still wakes promptly (either via the
// recheck observing freed budget, or via the queued notify).
#[tokio::test]
async fn test_reserve_no_lost_wakeup_in_wait_path() {
    let budget = Arc::new(PipelineBudget::new(0, 1));
    budget.set_multiplier(1.0);
    let full = budget.budget;

    // Saturate the budget so the next reserve is forced into rollback.
    budget.reserve(full).await;

    // Arm pause; the spawned reserver will signal `arrived` after
    // its rollback and then wait on `resume` before enqueuing.
    let pause = budget.arm_pause_hook();

    let reserver = {
        let budget = Arc::clone(&budget);
        tokio::spawn(async move { budget.reserve(1).await })
    };

    // Wait until the reserver is parked between rollback and enqueue.
    pause.arrived.notified().await;

    // Disarm the hook so any *future* reserve call (post-fix, when
    // the reserver retries) is not also trapped.
    *budget.test_pause_hook.lock() = None;

    // Release everything. The wait queue is empty here, so
    // `wake_next` is a no-op — this is the lost-wakeup window.
    budget.release(full);

    // Let the reserver continue past the pause. With the bug it now
    // pushes onto the wait queue and awaits a notify that will
    // never come; with the fix it must observe the freed budget and
    // succeed.
    pause.resume.notify_one();

    tokio::time::timeout(std::time::Duration::from_secs(1), reserver)
        .await
        .expect("reserve hung — lost-wakeup race in rollback→enqueue gap")
        .expect("reserver task panicked");
}

// --- ReservationGuard --------------------------------------------------

#[tokio::test]
async fn test_reservation_guard_commit_records_actual() {
    let budget = Arc::new(PipelineBudget::new(0, 1));
    budget.set_multiplier(1.0);

    let estimated = 1000;
    let actual = 800;
    let guard = budget.reserve_guarded(estimated).await;
    guard.commit(actual);

    // After commit, current should reflect the actual decoded size,
    // not the (1.0x) reserved amount.
    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        actual,
        "commit should reduce current to the actual decoded size",
    );
}

#[tokio::test]
async fn test_reservation_guard_drop_refunds_reservation() {
    let budget = Arc::new(PipelineBudget::new(0, 1));
    budget.set_multiplier(1.0);

    let estimated = 1000;
    let multiplier_before = f64::from_bits(
        budget
            .estimate_multiplier
            .load(std::sync::atomic::Ordering::Acquire),
    );

    // Drop without commit (simulates an early-return error path).
    {
        let _guard = budget.reserve_guarded(estimated).await;
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            estimated,
        );
    }

    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        0,
        "dropped guard should refund the entire reservation",
    );

    // Multiplier must NOT shift toward zero on a refund — a failed
    // fetch observed nothing about decode ratios.
    let multiplier_after = f64::from_bits(
        budget
            .estimate_multiplier
            .load(std::sync::atomic::Ordering::Acquire),
    );
    assert!(
        (multiplier_after - multiplier_before).abs() < 1e-9,
        "guard drop must not fold a (estimated, 0) sample into the EMA \
             (before={multiplier_before}, after={multiplier_after})",
    );
}

#[tokio::test]
async fn test_reservation_guard_drop_wakes_waiter() {
    let budget = Arc::new(PipelineBudget::new(0, 1));
    budget.set_multiplier(1.0);
    let full = budget.budget;

    // First reservation saturates the budget but is held by a guard.
    let blocking_guard = budget.reserve_guarded(full).await;
    assert_eq!(
        budget.current.load(std::sync::atomic::Ordering::Acquire),
        full,
    );

    // Spawn a waiter that wants 1 byte — should be parked.
    let budget_clone = Arc::clone(&budget);
    let waiter = tokio::spawn(async move { budget_clone.reserve(1).await });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!waiter.is_finished(), "second reserve should be parked");

    // Drop the guard without commit; the refund must wake the waiter.
    drop(blocking_guard);

    tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
        .await
        .expect("guard drop did not wake parked reserver")
        .expect("waiter task panicked");
}

/// An uncommitted guard drop must vacate the segment-count gate
/// slots it admitted, not just the reserved bytes. A failed fetch's
/// segments never reach the CPU worker, so the `CurrentStores` drop
/// that normally finalizes them never runs — without vacating here
/// they leak from the gate forever and, since the segment cap stays
/// engaged even with a wide-open byte budget, eventually wedge it
/// shut for every later reservation.
#[tokio::test]
async fn test_reservation_guard_drop_vacates_segments() {
    // Byte budget wide open so this isolates the segment-gate path.
    let budget = Arc::new(PipelineBudget::with_exact_budget(1 << 30));
    budget.set_multiplier(1.0);

    // Fill the gate to the cap with guarded reservations, then drop
    // them all uncommitted (simulating MAX_CONCURRENT_SEGMENTS failed
    // fetches on distinct segments).
    {
        let mut guards = Vec::new();
        for i in 0..MAX_CONCURRENT_SEGMENTS {
            guards.push(
                budget
                    .reserve_guarded_with_priority(1, ti(0), vec![format!("seg{i}")])
                    .await,
            );
        }
        assert_eq!(
            budget.active_segments.lock().all.len(),
            MAX_CONCURRENT_SEGMENTS,
            "all segments should be admitted before the drops",
        );
    }

    assert_eq!(
        budget.active_segments.lock().all.len(),
        0,
        "dropped guards must vacate every segment slot, not just the bytes",
    );

    // The gate is fully reusable: a fresh reservation that would have
    // been wedged out by the leaked slots is admitted immediately.
    let reserved = budget
        .reserve_with_priority(1, ti(0), &["fresh".to_owned()])
        .await;
    assert_eq!(reserved, 1);
    assert!(budget.active_segments.lock().all.contains("fresh"));
}

/// A guard drop must wake a reserver parked specifically on the
/// **segment-count** gate (not the byte gate) once the dropped
/// fetch's segment slot frees.
#[tokio::test]
async fn test_reservation_guard_drop_wakes_segment_gated_waiter() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(1 << 30));
    budget.set_multiplier(1.0);

    // Fill the segment cap; the last slot is held by a guard we'll
    // drop. Byte budget is huge so only the segment gate can block.
    for i in 0..(MAX_CONCURRENT_SEGMENTS - 1) {
        budget
            .reserve_with_priority(1, ti(0), &[format!("seg{i}")])
            .await;
    }
    let guard = budget
        .reserve_guarded_with_priority(1, ti(0), vec!["doomed".to_owned()])
        .await;

    // A new-segment reserver must park on the segment cap.
    let b = Arc::clone(&budget);
    let waiter = tokio::spawn(async move {
        b.reserve_with_priority(1, ti(0), &["late".to_owned()])
            .await
    });
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(
        !waiter.is_finished(),
        "control: reserver must park on the segment cap"
    );

    // Drop the doomed fetch's guard uncommitted; vacating its segment
    // slot must wake the parked reserver.
    drop(guard);
    tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
        .await
        .expect("guard drop did not wake the segment-gated reserver")
        .expect("waiter task panicked");
}

// --- env-var parsing helpers -------------------------------------------

const DEFAULT_BYTES: usize = 64 * 1024 * 1024;

#[test]
fn test_parse_bytes_accepts_iec_suffix() {
    assert_eq!(
        parse_bytes_or_default("TEST", "128MiB", DEFAULT_BYTES),
        128 * 1024 * 1024,
    );
    assert_eq!(
        parse_bytes_or_default("TEST", "1GiB", DEFAULT_BYTES),
        1024 * 1024 * 1024,
    );
    assert_eq!(
        parse_bytes_or_default("TEST", "512KiB", DEFAULT_BYTES),
        512 * 1024,
    );
}

#[test]
fn test_parse_bytes_accepts_si_suffix() {
    assert_eq!(
        parse_bytes_or_default("TEST", "100MB", DEFAULT_BYTES),
        100_000_000,
    );
    assert_eq!(
        parse_bytes_or_default("TEST", "2GB", DEFAULT_BYTES),
        2_000_000_000,
    );
}

#[test]
fn test_parse_bytes_accepts_bare_integer_as_bytes() {
    assert_eq!(
        parse_bytes_or_default("TEST", "67108864", DEFAULT_BYTES),
        64 * 1024 * 1024,
    );
}

#[test]
fn test_parse_bytes_rejects_zero() {
    assert_eq!(
        parse_bytes_or_default("TEST", "0", DEFAULT_BYTES),
        DEFAULT_BYTES,
    );
}

#[test]
fn test_parse_bytes_rejects_negative() {
    assert_eq!(
        parse_bytes_or_default("TEST", "-1", DEFAULT_BYTES),
        DEFAULT_BYTES,
    );
    assert_eq!(
        parse_bytes_or_default("TEST", "-1MB", DEFAULT_BYTES),
        DEFAULT_BYTES,
    );
}

#[test]
fn test_parse_bytes_rejects_non_numeric() {
    assert_eq!(
        parse_bytes_or_default("TEST", "not-a-number", DEFAULT_BYTES),
        DEFAULT_BYTES,
    );
}

#[test]
fn test_parse_bytes_rejects_unknown_suffix() {
    // Mb (megabit) is intentionally not a valid byte suffix.
    assert_eq!(
        parse_bytes_or_default("TEST", "10Mb", DEFAULT_BYTES),
        DEFAULT_BYTES,
    );
}

#[test]
fn test_parse_fraction_accepts_valid_range() {
    assert!((parse_fraction_or_default("TEST", "0.5", 0.25) - 0.5).abs() < 1e-12);
    assert!((parse_fraction_or_default("TEST", "1.0", 0.25) - 1.0).abs() < 1e-12);
    assert!((parse_fraction_or_default("TEST", "0.0001", 0.25) - 0.0001).abs() < 1e-12);
}

#[test]
fn test_parse_fraction_rejects_zero() {
    assert!((parse_fraction_or_default("TEST", "0.0", 0.25) - 0.25).abs() < 1e-12);
}

#[test]
fn test_parse_fraction_rejects_above_one() {
    assert!((parse_fraction_or_default("TEST", "1.5", 0.25) - 0.25).abs() < 1e-12);
}

#[test]
fn test_parse_fraction_rejects_negative() {
    assert!((parse_fraction_or_default("TEST", "-0.5", 0.25) - 0.25).abs() < 1e-12);
}

#[test]
fn test_parse_fraction_rejects_nan_and_inf() {
    assert!((parse_fraction_or_default("TEST", "NaN", 0.25) - 0.25).abs() < 1e-12);
    assert!((parse_fraction_or_default("TEST", "inf", 0.25) - 0.25).abs() < 1e-12);
}

#[test]
fn test_parse_fraction_rejects_non_numeric() {
    assert!((parse_fraction_or_default("TEST", "bogus", 0.25) - 0.25).abs() < 1e-12);
}

// -----------------------------------------------------------------
// PR C: budget extensions — priority-wake, segment-count gate,
// stall-detection circuit-breaker.

fn ti(t: i64) -> TimeInt {
    TimeInt::saturated_temporal_i64(t)
}

/// Parked waiters wake in earliest-`task_time_min`-first order
/// when budget is freed, even though they enqueued in the
/// opposite order. Confirms the priority heap supersedes the old
/// FIFO wake.
#[tokio::test]
async fn test_priority_wake_orders_by_task_time_min() {
    let half = 1024 * 1024;
    let budget = Arc::new(PipelineBudget::with_exact_budget(half * 2));
    budget.set_multiplier(1.0);

    // Saturate the budget so subsequent reservers park.
    budget.reserve_with_priority(half * 2, ti(0), &[]).await;

    use std::sync::atomic::{AtomicU8, Ordering};
    let order = Arc::new(AtomicU8::new(0));

    // Enqueue waiters in reverse priority order. Later-time first,
    // earlier-time second. The priority heap should still pick
    // the earlier-time one when the budget frees.
    let order_late = Arc::clone(&order);
    let b_late = Arc::clone(&budget);
    let late = tokio::spawn(async move {
        b_late.reserve_with_priority(half, ti(100), &[]).await;
        order_late.fetch_or(0b10, Ordering::AcqRel);
    });
    // Yield so `late` enqueues first.
    tokio::task::yield_now().await;

    let order_early = Arc::clone(&order);
    let b_early = Arc::clone(&budget);
    let early = tokio::spawn(async move {
        b_early.reserve_with_priority(half, ti(1), &[]).await;
        order_early.fetch_or(0b01, Ordering::AcqRel);
    });
    tokio::task::yield_now().await;

    // Free enough for exactly one waiter.
    budget.release(half);

    // Wait for the early-time task to complete; the late-time one
    // should still be parked.
    early.await.unwrap();
    assert_eq!(
        order.load(Ordering::Acquire) & 0b01,
        0b01,
        "early-time waiter must wake first"
    );
    assert_eq!(
        order.load(Ordering::Acquire) & 0b10,
        0,
        "late-time waiter must still be parked"
    );

    // Free the rest so the test cleans up.
    budget.release(half);
    late.await.unwrap();
}

/// A single `reserve_with_priority` call carrying multiple
/// `segment_ids` admits them all atomically, even when the
/// segment-count gate is already at the cap minus the new-segment
/// count. Admitting only a representative would let a fetch
/// stealth-open additional segments past `MAX_CONCURRENT_SEGMENTS`.
#[tokio::test]
async fn test_segment_count_gate_admits_atomically() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(1 << 30));
    budget.set_multiplier(1.0);

    // Take the first MAX_CONCURRENT_SEGMENTS slots one-by-one so
    // we know the gate is exactly full of distinct segments.
    for i in 0..MAX_CONCURRENT_SEGMENTS {
        budget
            .reserve_with_priority(1, ti(0), &[format!("seg{i}")])
            .await;
    }

    // Free one slot; only one new segment id should be admittable.
    let freed = format!("seg{}", MAX_CONCURRENT_SEGMENTS - 1);
    budget.publish_segment_finalized(&freed);

    // A multi-segment fetch that would push past the cap must
    // stay parked. We test this by polling once and asserting
    // it didn't complete.
    let b = Arc::clone(&budget);
    let three_new = tokio::spawn(async move {
        b.reserve_with_priority(
            1,
            ti(0),
            &["new-a".to_owned(), "new-b".to_owned(), "new-c".to_owned()],
        )
        .await;
    });
    // Give the spawn a chance to park.
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(
        !three_new.is_finished(),
        "fetch must park because admitting 3 new segments would push past the cap"
    );

    // Free the other two slots; now the 3-segment fetch can be
    // admitted atomically.
    for i in 0..(MAX_CONCURRENT_SEGMENTS - 1) {
        budget.publish_segment_finalized(&format!("seg{i}"));
    }
    three_new.await.unwrap();
}

/// `MAX_CONCURRENT_SEGMENTS` caps the steady-state count of
/// in-flight segments holding reservations regardless of byte
/// budget headroom.
#[tokio::test]
async fn test_segment_count_gate_caps_at_max() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(1 << 30));
    budget.set_multiplier(1.0);

    for i in 0..MAX_CONCURRENT_SEGMENTS {
        budget
            .reserve_with_priority(1, ti(0), &[format!("seg{i}")])
            .await;
    }
    assert_eq!(
        budget.active_segments.lock().all.len(),
        MAX_CONCURRENT_SEGMENTS
    );

    // One more must park.
    let b = Arc::clone(&budget);
    let parked = tokio::spawn(async move {
        b.reserve_with_priority(1, ti(0), &["overflow".to_owned()])
            .await;
    });
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(!parked.is_finished());

    // Vacate a slot; the parked waiter advances.
    budget.publish_segment_finalized("seg0");
    parked.await.unwrap();
}

/// After `STALL_EMPTY_EMIT_THRESHOLD` consecutive
/// `notify_empty_emit` calls *with the budget ≥
/// `STALL_SATURATION_THRESHOLD` saturated*, `force_overcommit`
/// flips on and subsequent `reserve` calls bypass both gates.
#[tokio::test]
async fn test_stall_breaker_fires_after_threshold() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(100));
    budget.set_multiplier(1.0);

    // Saturate the budget so the stall detector's saturation gate
    // would be satisfied.
    budget.reserve_with_priority(100, ti(0), &[]).await;

    // Below threshold: detector idle.
    for _ in 0..(STALL_EMPTY_EMIT_THRESHOLD - 1) {
        budget.notify_empty_emit();
    }
    assert!(
        !budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );

    // Cross the threshold: force_overcommit fires.
    budget.notify_empty_emit();
    assert!(
        budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );

    // While the flag is set, a reserve that would normally park
    // gets through immediately via the bypass.
    let extra = budget.reserve_with_priority(50, ti(0), &[]).await;
    assert_eq!(extra, 50);
}

/// `force_overcommit` must bypass the **segment-count** gate as
/// well as the byte gate — the stall it's designed to break can
/// be caused by the segment cap (e.g. 3 segments active, the
/// horizon-advancing chunk's segment is a 4th, parked on the
/// gate), so bypassing only bytes would still deadlock.
/// Bypass-admitted segments must land in `SegmentGate::bypass` so
/// the cap self-heals on finalize.
#[tokio::test]
async fn test_stall_breaker_bypasses_segment_count_gate() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(1 << 30));
    budget.set_multiplier(1.0);

    // Fill the segment cap with normal segments. Byte budget is
    // huge so this isolates the segment-gate path.
    for i in 0..MAX_CONCURRENT_SEGMENTS {
        budget
            .reserve_with_priority(1, ti(0), &[format!("seg{i}")])
            .await;
    }
    assert_eq!(
        budget.active_segments.lock().effective_len(),
        MAX_CONCURRENT_SEGMENTS
    );

    // A new-segment reserve normally parks at this point. Confirm.
    let b = Arc::clone(&budget);
    let blocked = tokio::spawn(async move {
        b.reserve_with_priority(1, ti(0), &["new-cap-blocked".to_owned()])
            .await
    });
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(
        !blocked.is_finished(),
        "control: cap-blocked reserve must park without the bypass"
    );

    // Trip the breaker. The parked reserver must now wake and
    // admit via the segment-gate bypass.
    budget
        .force_overcommit
        .store(true, std::sync::atomic::Ordering::Release);
    budget.wake_next();
    let reserved = blocked.await.unwrap();
    assert_eq!(reserved, 1);

    // Bypass-admitted segment is tracked so the cap self-heals
    // when it finalizes.
    let segments = budget.active_segments.lock();
    assert!(segments.all.contains("new-cap-blocked"));
    assert!(segments.bypass.contains("new-cap-blocked"));
}

/// The stall detector resets on any real progress
/// (`notify_row_emitted` or `release`).
#[tokio::test]
async fn test_stall_breaker_clears_on_progress() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(100));
    budget.set_multiplier(1.0);
    budget.reserve_with_priority(100, ti(0), &[]).await;

    for _ in 0..STALL_EMPTY_EMIT_THRESHOLD {
        budget.notify_empty_emit();
    }
    assert!(
        budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );

    budget.notify_row_emitted();
    assert!(
        !budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );
    assert_eq!(
        budget
            .empty_emit_count
            .load(std::sync::atomic::Ordering::Acquire),
        0
    );

    // Re-arm and clear via `release` this time.
    for _ in 0..STALL_EMPTY_EMIT_THRESHOLD {
        budget.notify_empty_emit();
    }
    assert!(
        budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );
    budget.release(50);
    assert!(
        !budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );
}

/// Below-saturation empty emits should *not* trip the breaker,
/// even if they pile up indefinitely. A query that genuinely has
/// nothing to emit (waiting on slow IO with the budget mostly
/// empty) shouldn't trigger the bypass.
#[tokio::test]
async fn test_stall_breaker_ignores_unsaturated_budget() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(100));
    budget.set_multiplier(1.0);
    // No `reserve` → current = 0, saturation = 0%.

    for _ in 0..(STALL_EMPTY_EMIT_THRESHOLD * 3) {
        budget.notify_empty_emit();
    }
    assert!(
        !budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );
}

/// Empty emits that pile up *before* the budget saturates must
/// not pre-load the counter so the first saturated cycle trips
/// the breaker. The counter must measure *consecutive saturated*
/// empty emits — anything else is a false-positive stall.
#[tokio::test]
async fn test_stall_breaker_does_not_carry_unsaturated_count_into_saturation() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(100));
    budget.set_multiplier(1.0);

    // Simulate slow-manifest startup: many empty emits while the
    // budget is empty. Without the saturation-first reset this
    // would pump `empty_emit_count` past the threshold.
    for _ in 0..(STALL_EMPTY_EMIT_THRESHOLD * 5) {
        budget.notify_empty_emit();
    }
    assert_eq!(
        budget
            .empty_emit_count
            .load(std::sync::atomic::Ordering::Acquire),
        0,
        "unsaturated empty emits must reset the counter, not accumulate"
    );

    // Now saturate the budget and emit one empty cycle. The
    // breaker MUST NOT trip — count is 1, not threshold + N.
    budget.reserve_with_priority(100, ti(0), &[]).await;
    budget.notify_empty_emit();
    assert!(
        !budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire),
        "first saturated empty emit must not trip the breaker"
    );
    assert_eq!(
        budget
            .empty_emit_count
            .load(std::sync::atomic::Ordering::Acquire),
        1,
    );

    // It still trips after `STALL_EMPTY_EMIT_THRESHOLD` consecutive
    // saturated empty emits, as the original design intends.
    for _ in 0..(STALL_EMPTY_EMIT_THRESHOLD - 1) {
        budget.notify_empty_emit();
    }
    assert!(
        budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );
}

/// Bypass-admitted segments must not pin the segment-count gate
/// closed after `force_overcommit` clears: their slots are tracked
/// as overflow so the effective cap restores immediately, and new
/// normal admissions can proceed up to
/// [`MAX_CONCURRENT_SEGMENTS`] while the bypass-admitted segments
/// are still in flight.
#[tokio::test]
async fn test_segment_cap_self_heals_after_bypass() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(1 << 30));
    budget.set_multiplier(1.0);

    // Fill the cap with normal segments.
    for i in 0..MAX_CONCURRENT_SEGMENTS {
        budget
            .reserve_with_priority(1, ti(0), &[format!("seg{i}")])
            .await;
    }
    assert_eq!(
        budget.active_segments.lock().effective_len(),
        MAX_CONCURRENT_SEGMENTS
    );

    // Trip the stall breaker and admit a new segment via bypass.
    budget
        .force_overcommit
        .store(true, std::sync::atomic::Ordering::Release);
    budget
        .reserve_with_priority(1, ti(0), &["bypass-a".to_owned()])
        .await;
    {
        let segments = budget.active_segments.lock();
        assert_eq!(segments.all.len(), MAX_CONCURRENT_SEGMENTS + 1);
        assert_eq!(segments.bypass.len(), 1);
        assert_eq!(segments.effective_len(), MAX_CONCURRENT_SEGMENTS);
    }

    // Clear the breaker as a real `release` would.
    budget
        .force_overcommit
        .store(false, std::sync::atomic::Ordering::Release);

    // A new-segment fetch still parks (cap is at MAX). Without the
    // bypass-tracking fix this would *also* be the case, so the
    // critical check is the next step.
    let b = Arc::clone(&budget);
    let parked = tokio::spawn(async move {
        b.reserve_with_priority(1, ti(0), &["after-bypass".to_owned()])
            .await;
    });
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
    assert!(!parked.is_finished());

    // Finalize ONE normal segment. Effective cap drops to MAX-1,
    // so the parked waiter advances — even though `all.len()` is
    // still MAX (bypass segment still in flight). Pre-fix this
    // would have stayed blocked because the cap check used
    // `all.len()` directly.
    budget.publish_segment_finalized("seg0");
    parked.await.unwrap();

    // Post-state: seg0 finalized (removed), after-bypass admitted
    // (added) → seg1, seg2, bypass-a, after-bypass in flight.
    // `all.len() = 4` exceeds the raw cap, but `effective_len = 3`
    // (== MAX) because bypass-a doesn't count against the cap.
    let segments = budget.active_segments.lock();
    assert_eq!(segments.all.len(), MAX_CONCURRENT_SEGMENTS + 1);
    assert_eq!(segments.bypass.len(), 1);
    assert_eq!(segments.effective_len(), MAX_CONCURRENT_SEGMENTS);
}

/// `wake_next` must drain cancelled (orphan) waiters and deliver
/// the wake to the next genuine parked waiter. Without the drain,
/// a low-`task_time_min` orphan at the top of the priority heap
/// would steal the wake from a higher-`task_time_min` real waiter
/// every time a release fires.
#[tokio::test]
async fn test_wake_next_skips_cancelled_orphans() {
    use std::sync::atomic::Ordering::Acquire;

    let budget = PipelineBudget::with_exact_budget(100);

    // Push a cancelled orphan with very low `task_time_min` (the
    // pathological case — it sits at the top of the heap) plus a
    // real parked waiter with a much higher `task_time_min`.
    let orphan_notify = Arc::new(Notify::new());
    let orphan_cancelled = Arc::new(AtomicBool::new(true));
    let real_notify = Arc::new(Notify::new());
    let real_cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut queue = budget.wait_queue.lock();
        queue.push(Reverse(PriorityWaiter {
            task_time_min: ti(1),
            seq: 0,
            notify: Arc::clone(&orphan_notify),
            cancelled: Arc::clone(&orphan_cancelled),
            reserved_bytes: 0,
            segment_ids: Vec::new(),
        }));
        queue.push(Reverse(PriorityWaiter {
            task_time_min: ti(1_000),
            seq: 1,
            notify: Arc::clone(&real_notify),
            cancelled: Arc::clone(&real_cancelled),
            reserved_bytes: 1,
            segment_ids: Vec::new(),
        }));
    }

    budget.wake_next();

    // Both popped: orphan drained, real waiter notified.
    assert_eq!(budget.wait_queue.lock().len(), 0);

    // `notify_one` stores a single permit on `real_notify`. A
    // `notified().await` should resolve immediately. Use timeout
    // to fail fast if the wake was lost.
    let real_received = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        real_notify.notified(),
    )
    .await;
    assert!(
        real_received.is_ok(),
        "real waiter must receive its wake despite the lower-priority orphan ahead of it"
    );
    // Sanity: orphan flag was the trigger, untouched after drain.
    assert!(orphan_cancelled.load(Acquire));
}

/// `adjust_reservation` shrink frees budget without a `release`.
/// It must also reset the stall detector — otherwise
/// `force_overcommit` stays armed across a now-unsaturated window
/// and the next `reserve` bypasses both gates with no real stall.
#[tokio::test]
async fn test_adjust_reservation_shrink_resets_stall_detector() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(100));
    budget.set_multiplier(1.0);

    // Saturate the budget and trip the breaker.
    let reserved = budget.reserve_with_priority(100, ti(0), &[]).await;
    for _ in 0..STALL_EMPTY_EMIT_THRESHOLD {
        budget.notify_empty_emit();
    }
    assert!(
        budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire)
    );

    // Fetch landed with much smaller actual than reserved — shrink.
    // Budget drops to ~30%, well below the saturation gate.
    budget.adjust_reservation(reserved, reserved, 30);

    assert!(
        !budget
            .force_overcommit
            .load(std::sync::atomic::Ordering::Acquire),
        "shrink that frees budget must clear `force_overcommit`"
    );
    assert_eq!(
        budget
            .empty_emit_count
            .load(std::sync::atomic::Ordering::Acquire),
        0,
        "shrink that frees budget must reset the empty-emit counter"
    );
}

/// A `release` must hand its freed bytes to the highest-priority
/// waiter that can *actually* use them — not to a higher-priority
/// waiter still blocked on the segment-count gate. Regression test
/// for the wake-stealing / priority-inversion path: pre-fix, the
/// segment-blocked earliest-time waiter swallowed the wake, failed
/// its recheck, re-parked without re-delegating, and the freed bytes
/// stranded — the byte-only later-time waiter starved until an
/// unrelated wake fired.
#[tokio::test]
async fn test_release_wakes_admittable_not_segment_blocked_waiter() {
    let budget = Arc::new(PipelineBudget::with_exact_budget(100));
    budget.set_multiplier(1.0);

    // Fill the segment gate (3 distinct) and saturate the bytes.
    budget
        .reserve_with_priority(60, ti(5), &["seg0".to_owned()])
        .await;
    budget
        .reserve_with_priority(20, ti(5), &["seg1".to_owned()])
        .await;
    budget
        .reserve_with_priority(20, ti(5), &["seg2".to_owned()])
        .await;

    // W1: earliest time, but wants a NEW (4th) segment → blocked on
    // the segment-count gate even once bytes free.
    let b1 = Arc::clone(&budget);
    let w1 = tokio::spawn(async move {
        b1.reserve_with_priority(10, ti(1), &["seg-new".to_owned()])
            .await
    });
    // W2: later time, wants bytes for an ALREADY-active segment →
    // only byte-blocked, admittable as soon as bytes free.
    let b2 = Arc::clone(&budget);
    let w2 = tokio::spawn(async move {
        b2.reserve_with_priority(40, ti(9), &["seg0".to_owned()])
            .await
    });

    // Let both park.
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    assert!(
        !w1.is_finished(),
        "control: W1 must park on the segment gate"
    );
    assert!(!w2.is_finished(), "control: W2 must park on bytes");

    // Free exactly enough bytes for W2. W1 has higher priority but
    // still can't pass the segment gate, so the wake must skip it
    // and land on W2.
    budget.release(40);

    tokio::time::timeout(std::time::Duration::from_secs(1), w2)
        .await
        .expect(
            "freed bytes must wake the admittable byte-only waiter, not the segment-blocked one",
        )
        .expect("W2 panicked");
    assert!(
        !w1.is_finished(),
        "segment-gate-blocked W1 must stay parked despite higher priority",
    );

    // Cleanup: free a segment slot and bytes so W1 can finish.
    budget.publish_segment_finalized("seg1");
    budget.release(60);
    tokio::time::timeout(std::time::Duration::from_secs(1), w1)
        .await
        .expect("W1 should finish once its segment slot and bytes free")
        .expect("W1 panicked");
}
