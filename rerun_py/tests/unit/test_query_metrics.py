from __future__ import annotations

import contextvars
import datetime
import threading
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any

import pytest
from rerun.experimental import MetricsCollector, QueryMetrics, query_metrics
from rerun.experimental._query_metrics import _active_collectors

if TYPE_CHECKING:
    from collections.abc import Iterator


# ---------------------------------------------------------------------------
# Fake handle infrastructure
#
# The `query_metrics()` context manager imports `_new_metrics_collector`
# lazily from `rerun_bindings` on every call. Monkeypatching that symbol on
# `rerun_bindings` is what gets picked up at scope entry — same pattern as
# `test_tracing_session.py`.
# ---------------------------------------------------------------------------


def _fake_query_metrics(**overrides: Any) -> SimpleNamespace:
    """
    Build a stand-in for the Rust-side `_QueryMetrics` PyO3 class.

    Only the attributes the Python wrapper reads in `_from_rust` need to be
    present; default values are chosen so the resulting `QueryMetrics`
    dataclass is internally consistent.
    """
    defaults: dict[str, Any] = {
        "dataset_id": "ds-unit",
        "query_chunks": 3,
        "query_segments": 1,
        "query_layers": 1,
        "query_columns": 4,
        "query_entities": 2,
        "query_bytes": 1024,
        "query_chunks_per_segment_min": 3,
        "query_chunks_per_segment_max": 3,
        "query_chunks_per_segment_mean": 3.0,
        "query_type": "full_scan",
        "primary_index_name": "time_2",
        "time_to_first_chunk_info": datetime.timedelta(microseconds=200),
        "filters_pushed_down": 1,
        "filters_applied_client_side": 0,
        "entity_path_narrowing_applied": True,
        "total_duration": datetime.timedelta(microseconds=500),
        "time_to_first_chunk": None,
        "error_kind": None,
        "direct_terminal_reason": None,
        "fetch_grpc_requests": 1,
        "fetch_grpc_bytes": 2048,
        "fetch_direct_requests": 0,
        "fetch_direct_bytes": 0,
        "fetch_direct_retries": 0,
        "fetch_direct_requests_retried": 0,
        "fetch_direct_retry_sleep": datetime.timedelta(0),
        "fetch_direct_max_attempt": 0,
        "fetch_direct_original_ranges": 0,
        "fetch_direct_merged_ranges": 0,
    }
    defaults.update(overrides)
    return SimpleNamespace(**defaults)


class _FakeHandle:
    """
    Stand-in for the Rust `_MetricsCollectorHandle`.

    Honors the contract the Python wrapper depends on:
    - `snapshot()` is non-destructive — returns a copy of the current buffer.
    - `drain()` returns the buffer and clears it.

    Tests poke `pending` directly to simulate snapshots arriving from the
    (here-absent) Rust capture path.
    """

    def __init__(self) -> None:
        self.pending: list[SimpleNamespace] = []
        self.drain_calls = 0
        self.snapshot_calls = 0

    def snapshot(self) -> list[SimpleNamespace]:
        self.snapshot_calls += 1
        return list(self.pending)

    def drain(self) -> list[SimpleNamespace]:
        self.drain_calls += 1
        out = list(self.pending)
        self.pending.clear()
        return out


@pytest.fixture
def install_fake_handles(monkeypatch: pytest.MonkeyPatch) -> Iterator[list[_FakeHandle]]:
    """
    Install a fake `_new_metrics_collector` that hands out fresh `_FakeHandle`s.

    Yields the list of handles that have been allocated, in the order they
    were requested. Each `with query_metrics()` scope pulls one handle.
    """
    import rerun_bindings  # noqa: TID251

    handles: list[_FakeHandle] = []

    def factory() -> _FakeHandle:
        h = _FakeHandle()
        handles.append(h)
        return h

    monkeypatch.setattr(rerun_bindings, "_new_metrics_collector", factory)
    yield handles


# ---------------------------------------------------------------------------
# C1. Empty scope → empty collector.
# ---------------------------------------------------------------------------


def test_empty_scope_yields_empty_collector(install_fake_handles: list[_FakeHandle]) -> None:
    with query_metrics() as m:
        assert isinstance(m, MetricsCollector)
        assert m.queries == []
        assert m.last_query() is None

    assert m.queries == []
    assert m.last_query() is None
    assert len(install_fake_handles) == 1


# ---------------------------------------------------------------------------
# C2. A pending Rust-side snapshot surfaces through `.queries` / `.last_query()`.
# ---------------------------------------------------------------------------


def test_fake_handle_populates_collector(install_fake_handles: list[_FakeHandle]) -> None:
    with query_metrics() as m:
        handle = install_fake_handles[-1]
        handle.pending.append(_fake_query_metrics(query_chunks=7, fetch_grpc_bytes=9_000))
        qs = m.queries

    assert len(qs) == 1
    assert isinstance(qs[0], QueryMetrics)
    assert qs[0].query_chunks == 7
    assert qs[0].fetch_grpc_bytes == 9_000
    assert qs[0].entity_path_narrowing_applied is True
    assert m.last_query() == qs[0]


# ---------------------------------------------------------------------------
# C3. After `__exit__`, the collector still surfaces queries.
# Regression guard: `drain()` is called on exit and the result kept on the
# Python side, so `.queries` keeps working past the `with` block.
# ---------------------------------------------------------------------------


def test_drain_on_exit_preserves_queries(install_fake_handles: list[_FakeHandle]) -> None:
    with query_metrics() as m:
        handle = install_fake_handles[-1]
        # Mid-scope: snapshot() is empty (no captures yet).
        assert m.queries == []
        # The Rust side surfaces a snapshot via the buffer between mid-scope
        # read and `__exit__`. The wrapper drains this on exit.
        handle.pending.append(_fake_query_metrics(query_chunks=11))

    # After exit, `.queries` must still return the drained snapshot.
    assert len(m.queries) == 1
    assert m.queries[0].query_chunks == 11

    # And it must continue to return the same content on repeated reads —
    # i.e. the post-exit path doesn't itself drain anything.
    assert m.queries[0].query_chunks == 11


# ---------------------------------------------------------------------------
# C4. `clear()` empties both the Rust handle buffer and the Python side.
# ---------------------------------------------------------------------------


def test_clear_empties_both_buffers(install_fake_handles: list[_FakeHandle]) -> None:
    with query_metrics() as m:
        handle = install_fake_handles[-1]
        handle.pending.append(_fake_query_metrics(query_chunks=1))
        handle.pending.append(_fake_query_metrics(query_chunks=2))
        assert len(m.queries) == 2

        m.clear()

        # Rust side drained.
        assert handle.pending == []
        # Python side also empty.
        assert m.queries == []
        assert m.last_query() is None


# ---------------------------------------------------------------------------
# C5. ImportError on bindings → inert collector + warning, no propagation.
# ---------------------------------------------------------------------------


def test_inert_fallback_on_import_error(
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """
    Yield an inert collector if the bindings are missing.

    If `rerun_bindings` is missing the symbol, the context manager logs a
    warning and yields an inert collector instead of raising. This matches
    `tracing_session`'s behavior for a missing telemetry stack.
    """
    import rerun_bindings  # noqa: TID251

    monkeypatch.delattr(rerun_bindings, "_new_metrics_collector", raising=False)

    with caplog.at_level("WARNING", logger="rerun"):
        with query_metrics() as m:
            assert m.queries == []
            assert m.last_query() is None

    assert m.queries == []
    assert any("query_metrics" in r.getMessage() for r in caplog.records), (
        f"expected a WARNING about query_metrics, got: {[r.getMessage() for r in caplog.records]}"
    )


# ---------------------------------------------------------------------------
# C6. Allocation failure → inert collector + log, no propagation.
# ---------------------------------------------------------------------------


def test_allocation_failure_yields_inert_collector(
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    import rerun_bindings  # noqa: TID251

    def boom() -> None:
        raise RuntimeError("simulated allocation failure")

    monkeypatch.setattr(rerun_bindings, "_new_metrics_collector", boom)

    with caplog.at_level("ERROR", logger="rerun"):
        with query_metrics() as m:
            assert m.queries == []
            assert m.last_query() is None

    # The wrapper logs `exception`, which records at ERROR level.
    assert any("query_metrics" in r.getMessage() for r in caplog.records), (
        f"expected an error log about query_metrics, got: {[r.getMessage() for r in caplog.records]}"
    )


# ---------------------------------------------------------------------------
# C7. ContextVar lifecycle: scope enter pushes, scope exit pops.
# ---------------------------------------------------------------------------


def test_context_var_pushes_and_pops(install_fake_handles: list[_FakeHandle]) -> None:
    assert _active_collectors.get() == ()

    with query_metrics():
        active = _active_collectors.get()
        assert len(active) == 1
        assert active[0] is install_fake_handles[0]

    # After exit the stack is back to its pre-scope value.
    assert _active_collectors.get() == ()


@pytest.mark.usefixtures("install_fake_handles")
def test_context_var_pops_on_exception() -> None:
    class _Boom(Exception):
        pass

    with pytest.raises(_Boom):
        with query_metrics():
            assert len(_active_collectors.get()) == 1
            raise _Boom

    # Even on early exit via exception, the ContextVar resets cleanly.
    assert _active_collectors.get() == ()


# ---------------------------------------------------------------------------
# C8. Nested `query_metrics()` scopes both end up on the stack; a query
# observed mid-inner-scope is visible to both via the ContextVar.
# ---------------------------------------------------------------------------


def test_nested_scopes_stack(install_fake_handles: list[_FakeHandle]) -> None:
    with query_metrics() as outer:
        outer_handle = install_fake_handles[-1]
        assert _active_collectors.get() == (outer_handle,)

        with query_metrics() as inner:
            inner_handle = install_fake_handles[-1]
            # Both collectors are on the stack while the inner scope is open.
            assert _active_collectors.get() == (outer_handle, inner_handle)

            # Simulate the Rust capture path: it reads the ContextVar and
            # fans the snapshot out to every collector currently active.
            snap = _fake_query_metrics(query_chunks=42)
            for h in _active_collectors.get():
                h.pending.append(snap)  # type: ignore[attr-defined]

            # The inner scope sees the snapshot mid-scope.
            inner_last = inner.last_query()
            assert inner_last is not None
            assert inner_last.query_chunks == 42

        # After the inner scope exits, only the outer is on the stack.
        assert _active_collectors.get() == (outer_handle,)

    # Both scopes should have seen the snapshot — fan-out is observable.
    outer_last = outer.last_query()
    assert outer_last is not None
    assert outer_last.query_chunks == 42
    inner_last = inner.last_query()
    assert inner_last is not None
    assert inner_last.query_chunks == 42


# ---------------------------------------------------------------------------
# C9. Sibling scopes in detached contexts do not pollute each other.
# A `query_metrics()` scope opened in one `contextvars.Context` is invisible
# to a sibling context — which is the whole point of moving off the global
# registry.
# ---------------------------------------------------------------------------


@pytest.mark.usefixtures("install_fake_handles")
def test_sibling_contexts_are_isolated() -> None:
    barrier_after_enter = threading.Event()
    barrier_before_exit = threading.Event()
    observed_in_thread: list[tuple[object, ...]] = []

    def worker() -> None:
        # No `contextvars.copy_context()` here — the raw thread inherits an
        # empty default ContextVar value. The parent's scope must be
        # invisible.
        observed_in_thread.append(_active_collectors.get())
        barrier_after_enter.set()
        barrier_before_exit.wait(timeout=5.0)

    t = threading.Thread(target=worker)

    with query_metrics():
        assert len(_active_collectors.get()) == 1
        t.start()
        barrier_after_enter.wait(timeout=5.0)
        # The worker thread observed the default empty stack, not the
        # parent's scope.
        assert observed_in_thread == [()]
        barrier_before_exit.set()

    t.join(timeout=5.0)


# ---------------------------------------------------------------------------
# C10. `contextvars.copy_context()` *does* carry the scope into a child task.
# ---------------------------------------------------------------------------


@pytest.mark.usefixtures("install_fake_handles")
def test_copy_context_inherits_scope() -> None:
    captured: list[tuple[object, ...]] = []

    def child() -> None:
        captured.append(_active_collectors.get())

    with query_metrics():
        ctx = contextvars.copy_context()
        ctx.run(child)

    # The child saw the same single-element stack as the parent.
    assert len(captured) == 1
    assert len(captured[0]) == 1


# ---------------------------------------------------------------------------
# C11. `.queries` is non-destructive — repeated reads return the same content.
# ---------------------------------------------------------------------------


def test_repeated_reads_are_non_destructive(install_fake_handles: list[_FakeHandle]) -> None:
    with query_metrics() as m:
        handle = install_fake_handles[-1]
        handle.pending.append(_fake_query_metrics(query_chunks=5))
        first = m.queries
        second = m.queries

    assert first == second
    assert len(first) == 1
    assert first[0].query_chunks == 5
