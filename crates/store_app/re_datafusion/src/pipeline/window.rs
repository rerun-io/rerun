//! The issuance window `W` — the pipeline's single flow-control knob.
//!
//! A byte-denominated semaphore: the fetch executor may hold at most `W`
//! estimated-decoded bytes that have been issued but not yet emitted/GC'd.
//! Acquired on issue, moved into per-segment [`SegmentLedger`]s on delivery,
//! and released on GC / segment completion / drop — always via RAII, so no
//! exit path can leak bytes.
//!
//! **Single-currency rule:** every acquire and every release is denominated
//! in *plan-estimate* bytes (`chunk_byte_size_uncompressed`, falling back to
//! the compressed `chunk_byte_len`). Measured store bytes are never mixed in:
//! a release can never exceed the matching acquisition, so the ledger cannot
//! over-release, and clamping at acquisition means it cannot wedge.
//!
//! **Forward progress** does not come from the semaphore's FIFO fairness.
//! Bytes are released only by emitting, and a segment cannot emit until it is
//! complete, so a segment that needs more of `W` than is free can end up
//! waiting on bytes only it could free. FIFO decides *who waits first*, which
//! is no help when the front of the queue is the one that must not wait at
//! all. [`IssuanceWindow::acquire_priority`] carries that guarantee instead:
//! the single batch the driver is blocked on takes its bytes unconditionally,
//! so the front always completes and therefore always advances. FIFO order
//! among the remaining waiters is then just a fairness nicety — delivery order
//! is fixed by the driver's `buffered(N)` regardless.
//!
//! Bytes granted past what the window held are an **overdraft**: they count
//! against `W` for exactly as long as they are alive (a dying lease burns its
//! permits against the debt before they reach the semaphore, and therefore
//! before any parked waiter is funded), and the squeeze is undone when they
//! die (see [`Overdraft`]) — so the window converges back to its full
//! capacity once no overdraft is outstanding.

use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

/// Byte-denominated issuance window. See the module docs.
pub(crate) struct IssuanceWindow {
    semaphore: Arc<Semaphore>,
    capacity_bytes: u64,
    debt: Arc<Overdraft>,
}

/// What [`IssuanceWindow::acquire_priority`] granted past the window over a
/// partition's lifetime.
///
/// `W` is the stated memory bound, but the priority path grants past it
/// unconditionally — so on a partition whose segments are larger than `W`,
/// `W` bounds nothing and there is otherwise no signal saying so. These are
/// the numbers that distinguish "`W` is set too high" from "`W` was never in
/// effect".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverdraftAudit {
    /// Priority acquisitions the window could not fully cover. Priority
    /// acquisitions it *could* cover are not overdrafts and are not counted.
    pub grants: u64,

    /// Σ unbacked bytes ever granted.
    pub granted_bytes: u64,

    /// High-water mark of unbacked bytes alive at once — how far past `W`
    /// residency actually went. Distinct from the debt counter, which drops
    /// as returning permits are burnt while those bytes are still alive.
    pub peak_live_bytes: u64,
}

/// Bookkeeping for bytes granted by [`IssuanceWindow::acquire_priority`]
/// beyond what the window held — shared between the window and every
/// [`WindowLease`] carrying unbacked bytes.
///
/// Lifecycle of one overdrafted byte: it is added to `owed` at grant, may be
/// repaid by burning a returning permit ([`Self::repay_from`],
/// [`Self::repay`]), and is settled when it dies ([`Self::settle`]) —
/// cancelled from `owed` if never burnt, or minted back as a fresh permit if
/// it was. Total minted can never exceed total burnt (a byte is either
/// cancelled or minted, and only burnt bytes escape cancellation), so the
/// semaphore never exceeds its capacity.
struct Overdraft {
    /// The same semaphore as [`IssuanceWindow::semaphore`] — repayment burns
    /// its permits, settlement mints them back.
    semaphore: Arc<Semaphore>,

    /// A mutex, not atomics: repayment must decide *how much* to burn and
    /// subtract it as one step, or two concurrent repayers burn against the
    /// same debt twice and the subtraction underflows. The lock is held only
    /// for a few arithmetic steps and never across an await. Grant and settlement already take it, so the audit
    /// counters ride along for free and stay exact.
    ledger: Mutex<OverdraftLedger>,
}

#[derive(Default)]
struct OverdraftLedger {
    /// Unbacked bytes not yet repaid or settled.
    owed: u64,

    /// Unbacked bytes currently alive. Unlike `owed` this does not fall when
    /// a returning permit is burnt — the bytes are still resident.
    live_bytes: u64,

    peak_live_bytes: u64,
    grants: u64,
    granted_bytes: u64,
}

impl Overdraft {
    /// Burn up to the outstanding debt out of `permit` before it returns to
    /// the semaphore.
    ///
    /// Must run on every lease drop: the semaphore hands returned permits
    /// straight to parked waiters, so debt that is not taken here would
    /// fund a waiter instead, and the overdraft would stop counting against
    /// `W` while its bytes are still alive.
    fn repay_from(&self, permit: &mut OwnedSemaphorePermit) {
        let mut ledger = self.ledger.lock();
        let take = ledger.owed.min(permit.num_permits() as u64);
        if take == 0 {
            return;
        }
        #[expect(clippy::cast_possible_truncation)] // ≤ num_permits, a usize
        if let Some(burnt) = permit.split(take as usize) {
            burnt.forget();
            ledger.owed -= take;
        }
    }

    /// Burn free permits against outstanding priority debt.
    ///
    /// Picks up permits that reach the semaphore without passing through a
    /// lease: those a cancelled gated acquire had been partially assigned.
    /// The semaphore offers them to other parked waiters first, so only what
    /// is left free is burnt here. Called before a gated acquire parks.
    fn repay(&self) {
        let mut ledger = self.ledger.lock();
        if ledger.owed == 0 {
            return;
        }
        // The lock spans the burn and the subtraction: `take` is decided
        // against a debt no concurrent repayer can also claim, so the
        // total burnt can never exceed the total owed. A concurrent gated
        // acquire may still win the counted permits — then `try_acquire_many`
        // fails and the debt simply stays owed.
        let take = ledger
            .owed
            .min(self.semaphore.available_permits() as u64)
            .min(u64::from(u32::MAX));
        if take == 0 {
            return;
        }
        #[expect(clippy::cast_possible_truncation)] // clamped to u32::MAX
        if let Ok(permit) = self.semaphore.try_acquire_many(take as u32) {
            permit.forget();
            ledger.owed -= take;
        }
    }

    /// Record `bytes` of unbacked grant. Returns nothing; the caller already
    /// holds the amount on its lease.
    fn grant(&self, bytes: u64) {
        let mut ledger = self.ledger.lock();
        ledger.owed += bytes;
        ledger.live_bytes += bytes;
        ledger.peak_live_bytes = ledger.peak_live_bytes.max(ledger.live_bytes);
        ledger.grants += 1;
        ledger.granted_bytes += bytes;
    }

    fn audit(&self) -> OverdraftAudit {
        let ledger = self.ledger.lock();
        OverdraftAudit {
            grants: ledger.grants,
            granted_bytes: ledger.granted_bytes,
            peak_live_bytes: ledger.peak_live_bytes,
        }
    }

    /// Settle `bytes` of dying unbacked lease: cancel what was never burnt,
    /// mint back what was.
    ///
    /// Without the minting arm, a repaid overdraft would shrink the window
    /// permanently — the burnt permits compensated for bytes that no longer
    /// exist, yet nothing returned them.
    fn settle(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }
        let mut ledger = self.ledger.lock();
        ledger.live_bytes = ledger.live_bytes.saturating_sub(bytes);
        let cancelled = bytes.min(ledger.owed);
        ledger.owed -= cancelled;
        let minted = bytes - cancelled;
        if minted > 0 {
            #[expect(clippy::cast_possible_truncation)] // ≤ burnt ≤ capacity ≤ u32::MAX
            self.semaphore.add_permits(minted as usize);
        }
    }
}

impl IssuanceWindow {
    /// A window of `capacity_bytes`. Clamped to at least one byte so that
    /// [`Self::acquire`]'s own clamping always has one admissible batch.
    ///
    /// Capacities beyond `u32::MAX` bytes are clamped, because
    /// [`Self::acquire`] clamps each request to the capacity before
    /// acquiring it, and a single acquisition counts permits in a `u32`.
    pub fn new(capacity_bytes: u64) -> Self {
        let capacity_bytes = capacity_bytes.clamp(1, u64::from(u32::MAX));
        #[expect(clippy::cast_possible_truncation)] // clamped above
        let semaphore = Arc::new(Semaphore::new(capacity_bytes as usize));
        Self {
            semaphore: Arc::clone(&semaphore),
            capacity_bytes,
            debt: Arc::new(Overdraft {
                semaphore,
                ledger: Mutex::default(),
            }),
        }
    }

    /// Acquire `bytes` for the batch the driver is currently blocked on,
    /// without ever waiting.
    ///
    /// That batch has to make progress or the partition wedges: its segment
    /// cannot emit — and therefore cannot release — until it is complete, and
    /// nothing else will release on its behalf. A segment whose estimate
    /// exceeds `W`, or one whose successors already hold the window, would
    /// otherwise block on a permit that can never arrive.
    ///
    /// Whatever the window cannot cover becomes unbacked bytes on the
    /// returned lease, owed to the window (see the module docs), which bounds
    /// peak residency at roughly `max(head segment, W)` rather than
    /// `head segment + W`.
    pub fn acquire_priority(&self, bytes: u64) -> WindowLease {
        // A concurrent acquire can only lower the free count between the read
        // and the take, so this retries with the new count and ends at zero
        // permits at worst — it never waits.
        let permit = loop {
            let available = self.semaphore.available_permits() as u64;
            let take = bytes.min(available).min(u64::from(u32::MAX));
            #[expect(clippy::cast_possible_truncation)] // clamped to u32::MAX
            match Arc::clone(&self.semaphore).try_acquire_many_owned(take as u32) {
                Ok(permit) => break permit,
                Err(TryAcquireError::NoPermits) => {}
                Err(TryAcquireError::Closed) => {
                    unreachable!("issuance window semaphore is never closed")
                }
            }
        };

        let amount = bytes - permit.num_permits() as u64;
        if amount > 0 {
            self.debt.grant(amount);
        }
        WindowLease {
            permit,
            unbacked: UnbackedBytes {
                amount,
                overdraft: Arc::clone(&self.debt),
            },
        }
    }

    /// Acquire `bytes` from the window, waiting until they are available.
    ///
    /// `bytes` is clamped to the window capacity so a single batch larger
    /// than `W` is admitted (with a warning) instead of deadlocking — the
    /// moral equivalent of "every channel has ≥ 1 buffer" in credit-based
    /// flow control.
    pub async fn acquire(&self, bytes: u64) -> WindowLease {
        self.debt.repay();
        let clamped = bytes.min(self.capacity_bytes);
        if clamped < bytes {
            re_log::warn_once!(
                "fetch batch estimate exceeds the issuance window; admitting anyway \
                 (batch={} bytes, window={} bytes). Consider raising the window size.",
                bytes,
                self.capacity_bytes,
            );
        }
        #[expect(clippy::cast_possible_truncation)] // capacity is clamped to u32::MAX
        let permit = Arc::clone(&self.semaphore)
            .acquire_many_owned(clamped as u32)
            .await
            .expect("issuance window semaphore is never closed");
        WindowLease {
            permit,
            unbacked: UnbackedBytes {
                amount: 0,
                overdraft: Arc::clone(&self.debt),
            },
        }
    }

    /// `W`, after the clamping [`Self::new`] applies.
    pub fn capacity_bytes(&self) -> u64 {
        self.capacity_bytes
    }

    /// What the priority path granted past `W`. See [`OverdraftAudit`].
    pub fn overdraft_audit(&self) -> OverdraftAudit {
        self.debt.audit()
    }

    /// Bytes currently available for issue. Test helper.
    #[cfg(test)]
    pub fn available_bytes(&self) -> u64 {
        self.semaphore.available_permits() as u64
    }
}

/// RAII over a byte quantity acquired from an [`IssuanceWindow`].
///
/// Dropping the lease refunds every byte it still holds, repaying any
/// outstanding overdraft first.
#[must_use = "dropping a WindowLease refunds its bytes immediately"]
pub(crate) struct WindowLease {
    permit: OwnedSemaphorePermit,
    unbacked: UnbackedBytes,
}

/// The overdrafted portion of a lease: bytes the window granted without
/// permits behind them.
///
/// Settled on drop, so every exit path — GC release,
/// segment completion, error unwind, tree cancellation — undoes its share
/// of the squeeze.
struct UnbackedBytes {
    amount: u64,
    overdraft: Arc<Overdraft>,
}

impl Drop for UnbackedBytes {
    fn drop(&mut self) {
        self.overdraft.settle(self.amount);
    }
}

impl Drop for WindowLease {
    fn drop(&mut self) {
        // Runs before the fields drop: the backed permits repay debt, then
        // what is left returns to the semaphore, then `UnbackedBytes`
        // settles this lease's own unbacked share.
        self.unbacked.overdraft.repay_from(&mut self.permit);
    }
}

impl WindowLease {
    /// Split off up to `bytes` into a new lease, clamped to what this lease
    /// still holds. Clamping (rather than panicking) is deliberate: batch
    /// leases may hold less than the sum of their chunks' estimates when the
    /// acquisition itself was clamped to the window capacity — under-holding
    /// is a soft bound, over-releasing is structurally impossible.
    ///
    /// Permit-backed bytes move first: they return to the window the moment
    /// the split lease dies, whereas unbacked bytes only settle debt.
    pub fn split(&mut self, bytes: u64) -> Self {
        let backed = bytes.min(self.permit.num_permits() as u64);
        #[expect(clippy::cast_possible_truncation)] // ≤ num_permits, a usize
        let permit = self
            .permit
            .split(backed as usize)
            .expect("split is clamped to the held quantity");
        let amount = (bytes - backed).min(self.unbacked.amount);
        self.unbacked.amount -= amount;
        Self {
            permit,
            unbacked: UnbackedBytes {
                amount,
                overdraft: Arc::clone(&self.unbacked.overdraft),
            },
        }
    }

    /// Absorb another lease's bytes into this one.
    pub fn merge(&mut self, mut other: Self) {
        // `other` implements `Drop`, so its permit is emptied by a full split
        // rather than moved out; the empty remainder drops as a no-op.
        if let Some(permit) = other.permit.split(other.permit.num_permits()) {
            self.permit.merge(permit);
        }
        self.unbacked.amount += std::mem::take(&mut other.unbacked.amount);
    }

    /// Bytes this lease currently holds, permit-backed plus unbacked.
    pub fn held_bytes(&self) -> u64 {
        self.permit.num_permits() as u64 + self.unbacked.amount
    }
}

/// Per-segment holder of window bytes: one merged lease covering every
/// delivered, not-yet-GC'd chunk of the segment — in estimate space.
///
/// Invariant: `held == Σ est(delivered chunks not yet released)`, modulo
/// acquisition clamping (see [`WindowLease::split`]). Every exit path —
/// segment finalize, error unwind, tree cancellation — is a `Drop`, which
/// refunds the residual, so the ledger cannot leak.
#[derive(Default)]
pub(crate) struct SegmentLedger {
    lease: Option<WindowLease>,
}

impl SegmentLedger {
    /// Move a delivered chunk's share of the batch lease into this segment.
    pub fn absorb(&mut self, lease: WindowLease) {
        match &mut self.lease {
            Some(held) => held.merge(lease),
            None => self.lease = Some(lease),
        }
    }

    /// Release `est_bytes` back to the window (clamped to what is held).
    /// Called with the plan-estimate sum of the chunks a GC pass removed.
    pub fn release(&mut self, est_bytes: u64) {
        if let Some(held) = &mut self.lease {
            drop(held.split(est_bytes));
            if held.held_bytes() == 0 {
                self.lease = None;
            }
        }
    }

    /// Bytes this ledger currently holds. Test helper.
    #[cfg(test)]
    pub fn held_bytes(&self) -> u64 {
        self.lease.as_ref().map_or(0, WindowLease::held_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acquire_and_drop_round_trip() {
        let window = IssuanceWindow::new(100);
        assert_eq!(window.available_bytes(), 100);

        let lease = window.acquire(40).await;
        assert_eq!(lease.held_bytes(), 40);
        assert_eq!(window.available_bytes(), 60);

        drop(lease);
        assert_eq!(window.available_bytes(), 100);
    }

    #[tokio::test]
    async fn oversized_acquire_clamps_to_capacity_instead_of_wedging() {
        let window = IssuanceWindow::new(10);

        // A batch bigger than the whole window must still be admissible.
        let lease = window.acquire(1_000).await;
        assert_eq!(lease.held_bytes(), 10);
        assert_eq!(window.available_bytes(), 0);

        drop(lease);
        assert_eq!(window.available_bytes(), 10);
    }

    #[tokio::test]
    async fn split_and_merge_conserve_bytes() {
        let window = IssuanceWindow::new(100);
        let mut lease = window.acquire(50).await;

        let part = lease.split(20);
        assert_eq!(part.held_bytes(), 20);
        assert_eq!(lease.held_bytes(), 30);
        assert_eq!(window.available_bytes(), 50);

        lease.merge(part);
        assert_eq!(lease.held_bytes(), 50);

        // Splitting more than held clamps instead of panicking.
        let all = lease.split(999);
        assert_eq!(all.held_bytes(), 50);
        assert_eq!(lease.held_bytes(), 0);
    }

    #[tokio::test]
    async fn ledger_release_and_drop_refund() {
        let window = IssuanceWindow::new(100);
        let mut lease = window.acquire(60).await;

        let mut ledger = SegmentLedger::default();
        ledger.absorb(lease.split(30));
        ledger.absorb(lease.split(30));
        drop(lease); // empty residual
        assert_eq!(ledger.held_bytes(), 60);
        assert_eq!(window.available_bytes(), 40);

        // Partial release (GC freed some chunks).
        ledger.release(25);
        assert_eq!(ledger.held_bytes(), 35);
        assert_eq!(window.available_bytes(), 65);

        // Over-release clamps to held.
        ledger.release(1_000);
        assert_eq!(ledger.held_bytes(), 0);
        assert_eq!(window.available_bytes(), 100);
    }

    #[tokio::test]
    async fn ledger_drop_refunds_residual() {
        let window = IssuanceWindow::new(100);
        {
            let mut ledger = SegmentLedger::default();
            ledger.absorb(window.acquire(70).await);
            assert_eq!(window.available_bytes(), 30);
        }
        assert_eq!(window.available_bytes(), 100);
    }

    /// An overdraft that dies before anything repaid it is cancelled
    /// outright — no permits burnt, capacity untouched.
    #[tokio::test]
    async fn unrepaid_overdraft_is_cancelled_on_drop() {
        let window = IssuanceWindow::new(100);
        let full = window.acquire(100).await;
        let overdraft = window.acquire_priority(50);
        assert_eq!(overdraft.held_bytes(), 50, "all unbacked");
        assert_eq!(window.available_bytes(), 0);

        drop(overdraft);
        assert_eq!(window.available_bytes(), 0, "nothing burnt, nothing minted");
        assert_eq!(window.debt.ledger.lock().owed, 0);

        drop(full);
        assert_eq!(window.available_bytes(), 100);
    }

    /// A lease that holds both permits and an overdraft repays its own debt
    /// out of its permits on drop, and the settlement mints them back.
    #[tokio::test]
    async fn self_repaying_overdraft_restores_the_window_on_drop() {
        let window = IssuanceWindow::new(100);
        let lease = window.acquire_priority(150);
        assert_eq!(lease.held_bytes(), 150, "100 permit-backed + 50 unbacked");
        assert_eq!(window.available_bytes(), 0);

        drop(lease);
        assert_eq!(window.available_bytes(), 100);
        assert_eq!(window.debt.ledger.lock().owed, 0);
    }

    /// Permits burnt against an overdraft are minted back when its unbacked
    /// bytes die, so the window converges to full capacity instead of
    /// shrinking by every overdraft it ever granted.
    #[tokio::test]
    async fn burnt_overdraft_is_minted_back_on_settle() {
        let window = IssuanceWindow::new(100);
        let mut lease = window.acquire_priority(150); // 100 backed + 50 unbacked

        // Part of the overdrafted segment GCs while the overdraft is still
        // alive: of 60 returning permit-backed bytes, 50 repay the debt.
        drop(lease.split(60));
        assert_eq!(window.available_bytes(), 10, "50 burnt, 10 returned");

        let gated = window.acquire(10).await;
        assert_eq!(window.available_bytes(), 0);
        drop(gated);
        assert_eq!(window.available_bytes(), 10);

        // The rest of the overdraft dies: 40 permits return and the 50
        // burnt ones are minted back.
        drop(lease);
        assert_eq!(window.available_bytes(), 100);
    }

    /// Concurrent repayers must not burn against the same debt twice.
    ///
    /// Each round releases a synchronized herd of threads into `repay`; the
    /// total burnt must equal the debt, never exceed it.
    #[test]
    fn concurrent_repayers_cannot_overpay_the_debt() {
        const CAPACITY: u64 = 512;
        const DEBT: u64 = 100;
        const THREADS: usize = 8;
        const ROUNDS: usize = 2_000;

        for _ in 0..ROUNDS {
            let window = Arc::new(IssuanceWindow::new(CAPACITY));
            // Outstanding debt with every permit free; no lease carries the
            // unbacked bytes, which `repay` never looks at.
            window.debt.ledger.lock().owed = DEBT;

            let barrier = Arc::new(std::sync::Barrier::new(THREADS));
            let repayers: Vec<_> = (0..THREADS)
                .map(|i| {
                    let window = Arc::clone(&window);
                    let barrier = Arc::clone(&barrier);
                    std::thread::Builder::new()
                        .name(format!("repayer-{i}"))
                        .spawn(move || {
                            barrier.wait();
                            window.debt.repay();
                        })
                        .expect("spawn repayer thread")
                })
                .collect();
            for repayer in repayers {
                repayer.join().unwrap();
            }

            let burnt = CAPACITY - window.available_bytes();
            assert_eq!(burnt, DEBT, "the whole debt is repayable in one pass");
            assert_eq!(
                window.debt.ledger.lock().owed,
                0,
                "…and must be repaid exactly once"
            );
        }
    }

    /// Returned permits repay outstanding debt before they reach a waiter
    /// that parked earlier.
    #[tokio::test]
    async fn returned_permits_repay_debt_before_a_parked_waiter() {
        let window = Arc::new(IssuanceWindow::new(100));
        let mut head = window.acquire_priority(150); // 100 backed + 50 owed

        let waiter = tokio::spawn({
            let window = Arc::clone(&window);
            async move { window.acquire(60).await }
        });
        tokio::task::yield_now().await; // let it park

        drop(head.split(60));
        tokio::task::yield_now().await;
        assert_eq!(
            window.debt.ledger.lock().owed,
            0,
            "50 of the 60 repay the debt"
        );
        assert!(
            !waiter.is_finished(),
            "the waiter must only have received the 10 bytes left after repayment",
        );

        drop(head); // 40 backed return and the 50 burnt are minted back
        let lease = waiter.await.expect("waiter task does not panic");
        assert_eq!(lease.held_bytes(), 60);
        drop(lease);
        assert_eq!(window.available_bytes(), 100);
    }

    /// A priority acquisition the window covers in full is not an overdraft
    /// and must not be counted as one — otherwise the audit says `W` was
    /// overruled on every partition, since the front batch always takes this
    /// path.
    #[tokio::test]
    async fn covered_priority_acquire_is_not_an_overdraft() {
        let window = IssuanceWindow::new(100);
        let lease = window.acquire_priority(40);

        assert_eq!(lease.held_bytes(), 40);
        assert_eq!(window.overdraft_audit(), OverdraftAudit::default());
    }

    /// The audit must survive settlement: the whole point is to report, after
    /// the fact, how far past `W` residency went. `peak_live_bytes` therefore
    /// tracks living unbacked bytes, not the debt counter — the debt falls as
    /// returning permits are burnt, while those bytes are still resident.
    #[tokio::test]
    async fn overdraft_audit_records_the_peak_and_outlives_settlement() {
        let window = IssuanceWindow::new(100);

        let mut first = window.acquire_priority(150); // 50 unbacked
        let second = window.acquire_priority(30); // window empty ⇒ all unbacked
        assert_eq!(
            window.overdraft_audit(),
            OverdraftAudit {
                grants: 2,
                granted_bytes: 80,
                peak_live_bytes: 80,
            }
        );

        // Burn the debt down without the bytes dying: the peak must not move.
        drop(first.split(60)); // 60 returning permit-backed bytes, all burnt
        assert_eq!(window.available_bytes(), 0, "60 of the 80 owed burnt");
        assert_eq!(window.overdraft_audit().peak_live_bytes, 80);

        drop(second);
        drop(first);
        assert_eq!(window.available_bytes(), 100, "window fully restored");
        assert_eq!(
            window.overdraft_audit(),
            OverdraftAudit {
                grants: 2,
                granted_bytes: 80,
                peak_live_bytes: 80,
            },
            "the audit is a lifetime record, not a gauge",
        );
    }

    /// FIFO fairness: with the window exhausted, the first waiter in plan
    /// order is served first when bytes free up.
    #[tokio::test]
    async fn waiters_are_served_in_fifo_order() {
        let window = Arc::new(IssuanceWindow::new(10));
        let first = window.acquire(10).await;

        let w = Arc::clone(&window);
        let second = tokio::spawn(async move { w.acquire(6).await });
        tokio::task::yield_now().await;
        let w = Arc::clone(&window);
        let third = tokio::spawn(async move { w.acquire(6).await });
        tokio::task::yield_now().await;

        drop(first);
        let second = second.await.unwrap();
        // `third` (6 bytes) cannot be served while `second` holds 6 of 10.
        assert!(!third.is_finished());
        drop(second);
        let third = third.await.unwrap();
        assert_eq!(third.held_bytes(), 6);
    }
}
