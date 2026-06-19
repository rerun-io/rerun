//! Customer-facing tracing-session correlation.
//!
//! See `rerun_py/rerun_sdk/rerun/_tracing_session.py` for the user-facing context
//! manager. The Rust side here owns the propagation pipeline:
//!
//! - The W3C `tracestate` key the session id rides under is
//!   [`RERUN_SESSION_TRACESTATE_KEY`].
//! - The `rs_<8-hex>` format is enforced by the [`RerunTracingSessionId`] newtype, whose
//!   only constructor [`RerunTracingSessionId::parse`] returns `None` on malformed input.
//!   Anything typed as `RerunTracingSessionId` past that boundary is by construction valid.
//! - The atomic gate ([`inc_active_tracing_session_count`] /
//!   [`dec_active_tracing_session_count`]) lets the per-RPC injection path skip
//!   invoking the [`SessionIdReader`] callback when nobody is opted in. The
//!   Python GIL is the motivating cost — under `rerun_py` the reader reaches
//!   into a Python `ContextVar` — but the gate is reader-agnostic.
//! - [`with_current_tracing_session`] calls the registered [`SessionIdReader`]
//!   once at the host-language→Rust boundary and stashes the value in a
//!   `tokio::task_local!` slot so every fan-out RPC inside the wrapped scope
//!   shares one reader call.
//! - [`current_rerun_session_id`] is the lookup the propagator
//!   (`TraceStateEnricher`) uses on every outbound gRPC injection.
//! - The `SessionIdReader` callback is supplied by the SDK binding via
//!   [`crate::Telemetry::init_with_session_id_reader`] (gated on the
//!   `session_id_reader` feature). Without it, this crate has no knowledge of
//!   where the active id lives — by design, so the crate doesn't need to pull
//!   in a host-language runtime (e.g. pyo3) just to read one string.

/// The W3C `tracestate` key under which the rerun session id propagates.
///
/// Server-side, `GrpcMakeSpan::make_span` reads this key and records the value as
/// the `rerun_session_id` span attribute, queryable in Tempo as
/// `{ .rerun_session_id = "…" }`.
pub const RERUN_SESSION_TRACESTATE_KEY: &str = "rerun_session_id";

/// A validated rerun session id.
///
/// The only constructor is [`RerunTracingSessionId::parse`], which enforces the
/// `rs_<8-hex>` format (e.g. `rs_cafebabe`). Holding a value of this type is a
/// compile-time guarantee that the contained string is well-formed: malformed
/// user input never pollutes server-side span attributes or outbound
/// `tracestate` headers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RerunTracingSessionId(String);

impl RerunTracingSessionId {
    /// Generate a fresh random session id of the form `rs_<8 lowercase hex>`.
    ///
    /// Module-private: the only public way to start a session is
    /// [`with_tracing_session`], which calls this internally.
    fn fresh() -> Self {
        let n: u32 = rand::random();
        Self(format!("rs_{n:08x}"))
    }

    /// Parse a string into a [`RerunTracingSessionId`].
    ///
    /// Accepts exactly `rs_` followed by 8 lowercase hex digits. Returns `None`
    /// for any other input (wrong prefix, wrong length, uppercase, non-hex).
    pub fn parse(s: &str) -> Option<Self> {
        let rest = s.strip_prefix("rs_")?;
        if rest.len() == 8
            && rest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Some(Self(s.to_owned()))
        } else {
            None
        }
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RerunTracingSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<RerunTracingSessionId> for String {
    fn from(id: RerunTracingSessionId) -> Self {
        id.0
    }
}

/// Process-wide counter of active tracing-session scopes.
///
/// Read on every outbound gRPC injection to short-circuit the [`SessionIdReader`]
/// call when nobody is opted in. Bumped from both entry points: Rust
/// [`with_tracing_session`] (via [`ActiveSessionGuard`]) and Python
/// `tracing_session().__enter__`/`__exit__`.
static ACTIVE_TRACING_SESSION_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Increment the active-session counter. Called by Python
/// `tracing_session().__enter__`; the Rust [`with_tracing_session`] path goes
/// through `ActiveSessionGuard` instead.
pub fn inc_active_tracing_session_count() {
    ACTIVE_TRACING_SESSION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Release);
}

/// Decrement the active-session counter. Called by Python
/// `tracing_session().__exit__`; the Rust [`with_tracing_session`] path goes
/// through `ActiveSessionGuard`'s drop instead.
pub fn dec_active_tracing_session_count() {
    ACTIVE_TRACING_SESSION_COUNT.fetch_sub(1, std::sync::atomic::Ordering::Release);
}

/// RAII handle on [`ACTIVE_TRACING_SESSION_COUNT`]: increments on construction,
/// decrements on drop — including drop during panic unwind. Used by Rust
/// scopes ([`with_tracing_session`], `scope_session_id_for_test`) so a panic
/// inside the wrapped future doesn't leak the counter and leave the atomic
/// gate stuck "active" for the rest of the process.
///
/// Python's `tracing_session().__exit__` runs on exception, so the Python
/// counterpart doesn't need this — the guard exists to match that behavior.
struct ActiveSessionGuard;

impl ActiveSessionGuard {
    fn new() -> Self {
        inc_active_tracing_session_count();
        Self
    }
}

impl Drop for ActiveSessionGuard {
    fn drop(&mut self) {
        dec_active_tracing_session_count();
    }
}

/// Callback signature for resolving the active session id from a
/// host-language store (e.g. a Python `ContextVar` in `rerun_py`).
///
/// Registered once at telemetry init via
/// [`crate::Telemetry::init_with_session_id_reader`]. Read on the slow path of
/// [`current_rerun_session_id`] and on the read-once at
/// [`with_current_tracing_session`].
#[cfg(feature = "session_id_reader")]
pub type SessionIdReader = fn() -> Option<RerunTracingSessionId>;

#[cfg(feature = "session_id_reader")]
static SESSION_ID_READER: std::sync::OnceLock<SessionIdReader> = std::sync::OnceLock::new();

/// Install the host-language session-id reader. First call wins. Subsequent
/// calls are silently ignored — registration is owned by whichever crate
/// initializes `Telemetry`.
#[cfg(feature = "session_id_reader")]
pub(crate) fn set_session_id_reader(reader: SessionIdReader) {
    // Result intentionally discarded: first-call-wins, subsequent attempts
    // are a silent no-op (see doc comment).
    SESSION_ID_READER.set(reader).ok();
}

/// Invoke the registered reader, if any. Returns `None` when no reader has
/// been installed or when the feature is off.
fn read_via_reader() -> Option<RerunTracingSessionId> {
    #[cfg(feature = "session_id_reader")]
    {
        let reader = SESSION_ID_READER.get()?;
        reader()
    }
    #[cfg(not(feature = "session_id_reader"))]
    {
        None
    }
}

// Per-tokio-task slot caching the rerun session id for the duration of a
// wrapped scope.
//
// Set once by `with_current_tracing_session` at the host-language→Rust boundary
// — typically a pyo3 catalog entry point in `rerun_py`, where calling the
// `SessionIdReader` means acquiring the GIL — and read on every outbound gRPC
// injection by `current_rerun_session_id` without re-invoking the reader.
// Propagates across `.await` within the same tokio task so DataFusion fan-out
// RPCs all share the value.
tokio::task_local! {
    static CURRENT_TRACING_SESSION_ID: Option<RerunTracingSessionId>;
}

/// Wrap `f` so the active rerun session id is resolved once at entry (via the
/// registered [`SessionIdReader`]) and stays accessible to every outbound gRPC
/// inside it without re-invoking the reader.
///
/// Used at every pyo3 catalog entry point in `rerun_py` to amortize the GIL
/// cost across the catalog method's fan-out.
#[must_use]
pub fn with_current_tracing_session<F>(
    f: F,
) -> tokio::task::futures::TaskLocalFuture<Option<RerunTracingSessionId>, F>
where
    F: std::future::Future,
{
    let sid = read_current_tracing_session_id_at_boundary();
    CURRENT_TRACING_SESSION_ID.scope(sid, f)
}

/// One-shot reader-callback invocation used by [`with_current_tracing_session`].
/// Gates on the atomic counter so the host-language store is never touched
/// when no scope is active.
fn read_current_tracing_session_id_at_boundary() -> Option<RerunTracingSessionId> {
    if ACTIVE_TRACING_SESSION_COUNT.load(std::sync::atomic::Ordering::Acquire) == 0 {
        return None;
    }
    read_via_reader()
}

/// Returns the active rerun session id, if any.
///
/// Source resolution, in order:
///
/// 1. Atomic gate: if no `tracing_session()` scope is active anywhere in the
///    process, return `None` immediately. One atomic load.
/// 2. tokio `task_local` set by [`with_current_tracing_session`] at the
///    host-language→Rust boundary: that value, possibly `None`, is
///    authoritative for the current task.
/// 3. Fallback: invoke the registered [`SessionIdReader`] callback (if any).
///    Only reached when the RPC fires outside any boundary helper (rare).
///
/// Returns `None` when no scope is active, the value fails
/// [`RerunTracingSessionId::parse`], or no reader has been registered (e.g.
/// the binary was built without the `session_id_reader` feature).
pub fn current_rerun_session_id() -> Option<RerunTracingSessionId> {
    if ACTIVE_TRACING_SESSION_COUNT.load(std::sync::atomic::Ordering::Acquire) == 0 {
        return None;
    }

    if let Ok(opt) = CURRENT_TRACING_SESSION_ID.try_with(|sid| sid.clone()) {
        return opt;
    }

    read_via_reader()
}

/// Test-only: scope `sid` into the task-local that [`current_rerun_session_id`]
/// reads, *and* bump the process-wide active-session counter so the atomic gate
/// doesn't short-circuit to `None`. Used by sibling crate modules (e.g. the
/// `RerunSessionRootSpanProcessor` tests in `tracestate.rs`) that need to
/// simulate an active `tracing_session()` scope without a Python interpreter.
#[cfg(test)]
pub(crate) async fn scope_session_id_for_test<F: std::future::Future>(
    sid: Option<RerunTracingSessionId>,
    f: F,
) -> F::Output {
    let _guard = ActiveSessionGuard::new();
    CURRENT_TRACING_SESSION_ID.scope(sid, f).await
}

/// Tag every Rerun Hub request inside `f` with a fresh session id, so the
/// full set of requests can be correlated end-to-end for support.
///
/// Two INFO log lines are emitted through the `tracing` stack — one on
/// entry, one on exit:
///
/// ```text
/// INFO rerun tracing session started: rs_8f3a91e2
/// …
/// INFO rerun tracing session finished rerun_session_id=rs_8f3a91e2 elapsed_s=12.345
/// ```
///
/// The "started" log fires the moment the scope is entered, so the id
/// stays visible even if the workflow crashes or hangs before completing.
/// Send that id to Rerun support and they can query
/// `{ .rerun_session_id = "rs_…" }` in our trace store to surface every
/// related request.
///
/// The "finished" log fires on normal return from `f` (whether it resolves
/// to `Ok` or `Err`) and includes the wall-clock duration. It is *skipped
/// if `f` panics* — the "started" log has already given the customer the
/// id, and a misleading "finished" log on a crash would just confuse.
///
/// Counterpart to Python's `tracing_session()` context manager. When you
/// also opt into exporting client-side traces (by setting
/// `RERUN_TELEMETRY_ENDPOINT`), those exported spans are tagged
/// with the same id, so the client→server trace tree stays correlated.
///
/// # Example
///
/// ```ignore
/// use re_perf_telemetry::with_tracing_session;
///
/// with_tracing_session(async {
///     let datasets = client.dataset_names().await?;
///     let ds = client.get_dataset("…").await?;
///     // …
/// })
/// .await;
/// ```
///
/// Nested calls work as you'd expect: an inner scope shadows the outer
/// session id while open, and the outer id is restored when the inner
/// scope exits.
///
/// # Getting the id programmatically
///
/// Most callers don't need the id in code — the INFO log is the
/// customer-facing way to retrieve it. If you do need it (e.g., to embed
/// in a support ticket emitted by your own logger), call
/// [`current_rerun_session_id`] from inside `f`:
///
/// ```ignore
/// use re_perf_telemetry::{current_rerun_session_id, with_tracing_session};
///
/// with_tracing_session(async {
///     let sid = current_rerun_session_id().expect("inside with_tracing_session");
///     my_logger::warn!("about to run a long workflow under session {sid}");
///     // …
/// })
/// .await;
/// ```
pub async fn with_tracing_session<F: std::future::Future>(f: F) -> F::Output {
    // No-op + warn if the telemetry stack hasn't been initialized. Without
    // it the propagator and span processor aren't installed, so outbound
    // requests wouldn't actually be tagged — running the full setup would
    // silently mislead the caller. Mirrors Python `tracing_session()`'s
    // no-op-with-warning branch.
    if !crate::is_telemetry_active() {
        tracing::warn!(
            "with_tracing_session is a no-op: the rerun telemetry stack is not active. \
             Call `Telemetry::init` first to enable session correlation."
        );
        return f.await;
    }

    let sid = RerunTracingSessionId::fresh();
    tracing::info!("rerun tracing session started: {sid}");
    // Participate in the process-wide active-scope counter (same as
    // Python's `__enter__`/`__exit__`) so `current_rerun_session_id`'s
    // atomic short-circuit correctly reflects that a session is active.
    // RAII so an `f` panic still decrements — see [`ActiveSessionGuard`].
    let _guard = ActiveSessionGuard::new();
    let t0 = std::time::Instant::now();
    let out = CURRENT_TRACING_SESSION_ID.scope(Some(sid.clone()), f).await;
    // Intentionally skipped on panic: if `f` panics the await unwinds and
    // this line never runs. The "started" log has already surfaced the id,
    // and a misleading "finished" log on a crash would just add noise. The
    // counter, by contrast, is decremented unconditionally via `_guard`.
    tracing::info!(
        rerun_session_id = %sid,
        elapsed_s = format!("{:.3}", t0.elapsed().as_secs_f64()),
        "rerun tracing session finished",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_ids() {
        assert!(RerunTracingSessionId::parse("").is_none());
        assert!(RerunTracingSessionId::parse("rs_").is_none());
        assert!(RerunTracingSessionId::parse("rs_cafebab").is_none()); // 7 hex chars
        assert!(RerunTracingSessionId::parse("rs_cafebabe1").is_none()); // 9 hex chars
        assert!(RerunTracingSessionId::parse("rs_CAFEBABE").is_none()); // uppercase rejected
        assert!(RerunTracingSessionId::parse("rs_cafebabz").is_none()); // non-hex
        assert!(RerunTracingSessionId::parse("xx_cafebabe").is_none()); // wrong prefix
        assert!(RerunTracingSessionId::parse("cafebabe").is_none()); // missing prefix
    }

    #[test]
    fn accepts_well_formed_id() {
        assert_eq!(
            RerunTracingSessionId::parse("rs_cafebabe")
                .unwrap()
                .as_str(),
            "rs_cafebabe",
        );
        assert!(RerunTracingSessionId::parse("rs_00000000").is_some());
        assert!(RerunTracingSessionId::parse("rs_ffffffff").is_some());
        assert!(RerunTracingSessionId::parse("rs_0123abcd").is_some());
    }

    /// `fresh()` must produce ids that round-trip through `parse`.
    ///
    /// Mirrors the Python `test_generated_id_is_valid` test on the
    /// `_generate_session_id` / `_is_valid_session_id` pair.
    #[test]
    fn fresh_generates_valid_id() {
        for _ in 0..16 {
            let sid = RerunTracingSessionId::fresh();
            assert!(
                RerunTracingSessionId::parse(&sid.to_string()).is_some(),
                "fresh() produced unparsable id: {sid}"
            );
        }
    }

    /// Nested `with_tracing_session` scopes shadow the outer id while open and
    /// restore it on exit. Mirrors the Python
    /// `test_nested_sessions_shadow_and_restore` test — same semantics, here
    /// implemented via `tokio::task_local::scope` instead of `ContextVar`
    /// token reset.
    #[test]
    fn nested_sessions_shadow_and_restore() {
        use parking_lot::Mutex;
        use std::sync::Arc;

        // `with_tracing_session` no-ops unless the telemetry stack is up; flip
        // the flag directly so this test exercises the active branch without
        // standing up the full OTel pipeline.
        crate::telemetry::set_telemetry_active_for_test(true);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let captures: Arc<Mutex<[Option<RerunTracingSessionId>; 3]>> =
            Arc::new(Mutex::new([None, None, None]));
        let captures_outer = Arc::clone(&captures);

        rt.block_on(super::with_tracing_session(async move {
            // 1: outer scope active
            captures_outer.lock()[0] = current_rerun_session_id();

            let captures_inner = Arc::clone(&captures_outer);
            super::with_tracing_session(async move {
                // 2: inner scope shadows outer
                captures_inner.lock()[1] = current_rerun_session_id();
            })
            .await;

            // 3: outer restored after inner exits
            captures_outer.lock()[2] = current_rerun_session_id();
        }));

        let captures = captures.lock();
        let outer = captures[0].clone().expect("outer scope should be active");
        let inner = captures[1].clone().expect("inner scope should be active");
        let after_inner = captures[2]
            .clone()
            .expect("outer should be restored after inner exit");
        drop(captures);

        assert_ne!(outer, inner, "nested scope should generate a distinct id");
        assert_eq!(
            outer, after_inner,
            "outer id should be restored after inner exits"
        );

        // 4: session fully cleared after outermost exits
        assert!(
            current_rerun_session_id().is_none(),
            "session should be cleared after outermost exits"
        );
    }

    /// Sanity: the active-session gate starts at zero and round-trips inc/dec.
    #[test]
    fn gate_inc_dec_round_trips() {
        use std::sync::atomic::Ordering;

        // Fresh process: counter is zero.
        assert_eq!(ACTIVE_TRACING_SESSION_COUNT.load(Ordering::Acquire), 0);
        inc_active_tracing_session_count();
        assert_eq!(ACTIVE_TRACING_SESSION_COUNT.load(Ordering::Acquire), 1);
        dec_active_tracing_session_count();
        assert_eq!(ACTIVE_TRACING_SESSION_COUNT.load(Ordering::Acquire), 0);
    }

    /// A panic inside `with_tracing_session`'s body must not leak the
    /// active-session counter. Without the RAII guard the atomic gate would
    /// stay stuck "active" and every subsequent `current_rerun_session_id`
    /// call in the process would skip the fast path forever.
    #[test]
    fn counter_balanced_on_panic_in_body() {
        use std::panic::AssertUnwindSafe;
        use std::sync::atomic::Ordering;

        crate::telemetry::set_telemetry_active_for_test(true);

        let baseline = ACTIVE_TRACING_SESSION_COUNT.load(Ordering::Acquire);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        #[expect(clippy::disallowed_methods, reason = "tests compile with panic=unwind")]
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            rt.block_on(super::with_tracing_session(async {
                panic!("boom");
            }));
        }));
        assert!(result.is_err(), "panic should have propagated");

        assert_eq!(
            ACTIVE_TRACING_SESSION_COUNT.load(Ordering::Acquire),
            baseline,
            "active-session counter must return to baseline after panic",
        );
    }
}
