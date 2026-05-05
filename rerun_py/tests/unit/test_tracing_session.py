from __future__ import annotations

import pytest
from rerun._tracing_session import (
    _generate_session_id,
    _is_valid_session_id,
    tracing_session,
)


def test_generated_id_is_valid() -> None:
    for _ in range(8):
        sid = _generate_session_id()
        assert _is_valid_session_id(sid), f"generated invalid id: {sid!r}"


def test_validation_rejects_malformed_ids() -> None:
    bad_ids = [
        "",
        "rs_",
        "rs_cafebab",  # 7 hex chars
        "rs_cafebabe1",  # 9 hex chars
        "rs_CAFEBABE",  # uppercase
        "rs_cafebabz",  # non-hex
        "xx_cafebabe",  # wrong prefix
        "cafebabe",  # missing prefix
    ]
    for sid in bad_ids:
        assert not _is_valid_session_id(sid), f"unexpectedly accepted: {sid!r}"


def test_logs_session_id_at_scope_entry(monkeypatch: pytest.MonkeyPatch) -> None:
    """The session id must be forwarded to the Rust `tracing` stack on scope entry."""
    import rerun_bindings  # noqa: TID251

    captured: list[str] = []

    def fake_log(sid: str) -> None:
        captured.append(sid)

    # The context manager imports its bindings lazily from `rerun_bindings`, so
    # patching the symbols on that module is what gets picked up at scope entry.
    # `_is_telemetry_active` is forced to `True` so the test exercises the
    # active-telemetry branch regardless of whether `TELEMETRY_ENABLED=true`
    # was set for the running process (CI does not set it).
    monkeypatch.setattr(rerun_bindings, "_is_telemetry_active", lambda: True)
    monkeypatch.setattr(rerun_bindings, "_log_tracing_session_started", fake_log)
    monkeypatch.setattr(rerun_bindings, "_log_tracing_session_finished", lambda *_args: None)

    with tracing_session() as sid:
        pass

    assert captured == [sid], f"expected Rust logger to be called once with {sid!r}, got: {captured!r}"


def test_logs_metrics_at_scope_exit(monkeypatch: pytest.MonkeyPatch) -> None:
    """A normal exit must invoke `_log_tracing_session_finished` once with the active session id."""
    import rerun_bindings  # noqa: TID251

    finished_calls: list[tuple[object, ...]] = []

    monkeypatch.setattr(rerun_bindings, "_is_telemetry_active", lambda: True)
    monkeypatch.setattr(rerun_bindings, "_log_tracing_session_started", lambda _sid: None)
    monkeypatch.setattr(
        rerun_bindings,
        "_log_tracing_session_finished",
        lambda *args: finished_calls.append(args),
    )

    with tracing_session() as sid:
        pass

    assert len(finished_calls) == 1, f"expected one finished call, got: {finished_calls!r}"
    args = finished_calls[0]
    # Signature: (sid, elapsed_s, cpu_user_s, cpu_system_s, cpu_iowait_s, net_rx_mb)
    assert args[0] == sid
    assert isinstance(args[1], float) and args[1] >= 0.0
    # Remaining four fields are float|None depending on psutil/platform availability.
    for field in args[2:]:
        assert field is None or isinstance(field, float)


def test_skips_metrics_log_when_block_raises(monkeypatch: pytest.MonkeyPatch) -> None:
    """If the `with` body raises, the finished-log must not fire (simpler control flow)."""
    import rerun_bindings  # noqa: TID251

    finished_calls: list[tuple[object, ...]] = []

    monkeypatch.setattr(rerun_bindings, "_is_telemetry_active", lambda: True)
    monkeypatch.setattr(rerun_bindings, "_log_tracing_session_started", lambda _sid: None)
    monkeypatch.setattr(
        rerun_bindings,
        "_log_tracing_session_finished",
        lambda *args: finished_calls.append(args),
    )

    class _Boom(Exception):
        pass

    with pytest.raises(_Boom):
        with tracing_session():
            raise _Boom

    assert finished_calls == [], f"expected no finished call on early exit, got: {finished_calls!r}"


def test_psutil_failure_does_not_propagate(monkeypatch: pytest.MonkeyPatch) -> None:
    """A psutil failure during snapshot or delta must never break the `with` block."""
    import rerun._tracing_session as ts

    import rerun_bindings  # noqa: TID251

    finished_calls: list[tuple[object, ...]] = []

    monkeypatch.setattr(rerun_bindings, "_is_telemetry_active", lambda: True)
    monkeypatch.setattr(rerun_bindings, "_log_tracing_session_started", lambda _sid: None)
    monkeypatch.setattr(
        rerun_bindings,
        "_log_tracing_session_finished",
        lambda *args: finished_calls.append(args),
    )

    class _BrokenPsutil:
        @staticmethod
        def Process() -> None:
            raise OSError("simulated AccessDenied")

        @staticmethod
        def net_io_counters() -> None:
            raise OSError("simulated permission failure")

    monkeypatch.setattr(ts, "psutil", _BrokenPsutil)

    body_ran = False
    with tracing_session() as sid:
        body_ran = True

    assert body_ran, "with-block body must execute even when psutil snapshots fail"
    assert len(finished_calls) == 1, f"expected one finished call, got: {finished_calls!r}"
    args = finished_calls[0]
    assert args[0] == sid
    assert isinstance(args[1], float)
    # All four metric fields must be None when psutil fails.
    assert args[2:] == (None, None, None, None)


def test_finished_log_failure_does_not_propagate(monkeypatch: pytest.MonkeyPatch) -> None:
    """If `_log_tracing_session_finished` itself raises, the `with` block must still complete cleanly."""
    import rerun_bindings  # noqa: TID251

    def boom(*_args: object) -> None:
        raise RuntimeError("simulated tracing failure")

    monkeypatch.setattr(rerun_bindings, "_is_telemetry_active", lambda: True)
    monkeypatch.setattr(rerun_bindings, "_log_tracing_session_started", lambda _sid: None)
    monkeypatch.setattr(rerun_bindings, "_log_tracing_session_finished", boom)

    # No exception should escape the context manager.
    with tracing_session() as sid:
        assert sid.startswith("rs_")


def test_warns_and_no_ops_when_telemetry_inactive(
    caplog: pytest.LogCaptureFixture,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Warn and yield an empty id when `TELEMETRY_ENABLED` is not truthy."""
    import rerun_bindings  # noqa: TID251

    # Force the inactive-telemetry branch regardless of process-wide env so
    # this case exercises in CI even when telemetry happens to be active.
    monkeypatch.setattr(rerun_bindings, "_is_telemetry_active", lambda: False)

    with caplog.at_level("WARNING", logger="rerun"):
        with tracing_session() as sid:
            assert sid == ""

    assert any("TELEMETRY_ENABLED=true" in r.getMessage() for r in caplog.records), (
        f"expected a WARNING about TELEMETRY_ENABLED, got: {[r.getMessage() for r in caplog.records]}"
    )
