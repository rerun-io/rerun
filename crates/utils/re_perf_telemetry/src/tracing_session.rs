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
//!   the Python GIL when nobody is opted in.
//! - [`with_current_tracing_session`] reads the Python `ContextVar` once at the
//!   Python→Rust boundary and stashes the value in a `tokio::task_local!` slot
//!   so every fan-out RPC inside the catalog method shares one GIL acquisition.
//! - [`current_rerun_session_id`] is the lookup the propagator
//!   (`TraceStateEnricher`) uses on every outbound gRPC injection.

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

/// Process-wide counter of active `tracing_session()` scopes.
///
/// Read on every outbound gRPC injection to short-circuit the GIL acquisition when
/// nobody is opted in. Incremented on `__enter__`, decremented on `__exit__` from
/// the Python `tracing_session()` context manager.
static ACTIVE_TRACING_SESSION_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Increment the active-session counter. Called by `tracing_session().__enter__`.
pub fn inc_active_tracing_session_count() {
    ACTIVE_TRACING_SESSION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Release);
}

/// Decrement the active-session counter. Called by `tracing_session().__exit__`.
pub fn dec_active_tracing_session_count() {
    ACTIVE_TRACING_SESSION_COUNT.fetch_sub(1, std::sync::atomic::Ordering::Release);
}

// Per-tokio-task slot caching the rerun session id for the duration of a Python
// catalog call.
//
// Set once at the Python→Rust boundary by `with_current_tracing_session` (one GIL
// acquisition per catalog method). Read on every outbound gRPC injection by
// `current_rerun_session_id` without touching Python. Propagates across `.await`
// within the same tokio task so DataFusion fan-out RPCs all share the value.
tokio::task_local! {
    static CURRENT_TRACING_SESSION_ID: Option<RerunTracingSessionId>;
}

/// Wrap `f` so the active rerun session id (read once from Python) is accessible
/// without GIL on every outbound gRPC inside it.
///
/// Used at every pyo3 catalog entry point in `rerun_py` to amortize the GIL cost
/// across the catalog method's fan-out.
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

/// One-shot `ContextVar` read used by [`with_current_tracing_session`]. Gates on the atomic
/// counter so the GIL is never touched when no scope is active.
fn read_current_tracing_session_id_at_boundary() -> Option<RerunTracingSessionId> {
    if ACTIVE_TRACING_SESSION_COUNT.load(std::sync::atomic::Ordering::Acquire) == 0 {
        return None;
    }
    read_current_tracing_session_id_via_pyo3()
}

/// Returns the active rerun session id, if any.
///
/// Source resolution, in order:
///
/// 1. Atomic gate: if no `tracing_session()` scope is active anywhere in the
///    process, return `None` immediately. One atomic load, no GIL.
/// 2. tokio `task_local` set by [`with_current_tracing_session`] at the Python→Rust
///    boundary: that value, possibly `None`, is authoritative for the current
///    task. Reads without touching Python.
/// 3. Fallback: read the Python `ContextVar` under the GIL. Only reached when
///    the RPC fires outside any pyo3 boundary helper (rare).
///
/// Returns `None` when no scope is active, the value fails [`RerunTracingSessionId::parse`],
/// or the binary was built without the `pyo3` feature.
pub fn current_rerun_session_id() -> Option<RerunTracingSessionId> {
    if ACTIVE_TRACING_SESSION_COUNT.load(std::sync::atomic::Ordering::Acquire) == 0 {
        return None;
    }

    if let Ok(opt) = CURRENT_TRACING_SESSION_ID.try_with(|sid| sid.clone()) {
        return opt;
    }

    read_current_tracing_session_id_via_pyo3()
}

/// pyo3 path for the `tracing_session()` `ContextVar` lookup.
///
/// Gated on `not(test)` because cargo's lib-test binary is a regular ELF
/// executable that has to resolve every `Py*` symbol at link time. Under
/// `--all-features`, `rerun_py/extension-module` enables `pyo3/extension-module`
/// for the whole build, which suppresses `cargo:rustc-link-lib=python3.*`. Any
/// reachable `pyo3::*` call in the lib test binary then fails to link with
/// `undefined symbol: PyImport_Import` and friends. Compiling this body out for
/// `cfg(test)` lets `--gc-sections` drop the entire pyo3 dependency from the
/// test binary while keeping it intact for the real (non-test) build that
/// `rerun_py` links into.
#[cfg(all(feature = "pyo3", not(test)))]
fn read_current_tracing_session_id_via_pyo3() -> Option<RerunTracingSessionId> {
    pyo3::Python::attach(crate::python_bridge::current_rerun_session_id_from_contextvar)
}

#[cfg(any(not(feature = "pyo3"), test))]
fn read_current_tracing_session_id_via_pyo3() -> Option<RerunTracingSessionId> {
    None
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

    /// Sanity: the active-session gate starts at zero and round-trips inc/dec.
    ///
    /// Intentionally does not call `current_rerun_session_id`: under the `pyo3`
    /// feature that function references Python C symbols which would force the
    /// test binary to link against the Python static library — which we don't
    /// have on the regular `cargo nextest --all-features` path.
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
}
