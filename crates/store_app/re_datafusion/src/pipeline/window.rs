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
//! permits can only be dropped, never minted, so the ledger cannot
//! over-release, and clamping at acquisition means it cannot wedge.
//!
//! FIFO fairness of `tokio::sync::Semaphore` is load-bearing: under
//! contention the front-of-plan batch always gets bytes first, which is what
//! keeps "issuance order = emission order" (and with it the pipeline's
//! deadlock-freedom argument) true.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Byte-denominated issuance window. See the module docs.
pub(crate) struct IssuanceWindow {
    semaphore: Arc<Semaphore>,
    capacity_bytes: u64,
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
        Self {
            #[expect(clippy::cast_possible_truncation)] // clamped above
            semaphore: Arc::new(Semaphore::new(capacity_bytes as usize)),
            capacity_bytes,
        }
    }

    /// Acquire `bytes` from the window, waiting until they are available.
    ///
    /// `bytes` is clamped to the window capacity so a single batch larger
    /// than `W` is admitted (with a warning) instead of deadlocking — the
    /// moral equivalent of "every channel has ≥ 1 buffer" in credit-based
    /// flow control.
    pub async fn acquire(&self, bytes: u64) -> WindowLease {
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
        WindowLease { permit }
    }

    /// Bytes currently available for issue. Test/diagnostic helper.
    pub fn available_bytes(&self) -> u64 {
        self.semaphore.available_permits() as u64
    }
}

/// RAII over a byte quantity acquired from an [`IssuanceWindow`].
/// Dropping the lease refunds every byte it still holds.
#[must_use = "dropping a WindowLease refunds its bytes immediately"]
pub(crate) struct WindowLease {
    permit: OwnedSemaphorePermit,
}

impl WindowLease {
    /// Split off up to `bytes` into a new lease, clamped to what this lease
    /// still holds. Clamping (rather than panicking) is deliberate: batch
    /// leases may hold less than the sum of their chunks' estimates when the
    /// acquisition itself was clamped to the window capacity — under-holding
    /// is a soft bound, over-releasing is structurally impossible.
    pub fn split(&mut self, bytes: u64) -> Self {
        let n = bytes.min(self.held_bytes());
        #[expect(clippy::cast_possible_truncation)] // held bytes never exceed u32::MAX
        let permit = self
            .permit
            .split(n as usize)
            .expect("split is clamped to the held quantity");
        Self { permit }
    }

    /// Absorb another lease's bytes into this one.
    pub fn merge(&mut self, other: Self) {
        self.permit.merge(other.permit);
    }

    /// Bytes this lease currently holds.
    pub fn held_bytes(&self) -> u64 {
        self.permit.num_permits() as u64
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

    /// Bytes this ledger currently holds. Test/diagnostic helper.
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
