from __future__ import annotations

import os
import subprocess
from datetime import datetime, timezone
from unittest.mock import Mock, call

import datafusion
import pyarrow as pa
import pytest
from rerun._arrow import to_record_batch
from rerun.experimental import ViewerClient, _viewer_client
from rerun.experimental._viewer_client import _viewer_state_from_json

import rerun_bindings  # noqa: TID251


def _capturing_viewer(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[ViewerClient, list[tuple[str | None, int, bool, str | None]]]:
    calls: list[tuple[str | None, int, bool, str | None]] = []

    class CapturingViewerClientInternal:
        def __init__(self, _url: str) -> None:
            pass

        def set_time_cursor(self, timeline: str | None, time: int, play: bool, store_id: str | None = None) -> None:
            calls.append((timeline, time, play, store_id))

    monkeypatch.setattr(rerun_bindings, "ViewerClientInternal", CapturingViewerClientInternal)
    return ViewerClient.connect(), calls


def test_close_without_waiting(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, _calls = _capturing_viewer(monkeypatch)
    viewer._pid = 123
    terminate, wait = Mock(), Mock()
    monkeypatch.setattr(_viewer_client, "_wait_for_processes", wait)
    if os.name == "posix":
        monkeypatch.setattr(os, "killpg", terminate)
    else:
        monkeypatch.setattr(subprocess, "run", terminate)

    viewer.close(wait=False)

    terminate.assert_called_once()
    wait.assert_not_called()
    assert viewer._pid is None


def test_close_reports_failed_teardown(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, _calls = _capturing_viewer(monkeypatch)
    launcher = Mock(pid=123)
    launcher.children.return_value = []
    viewer._pid = launcher.pid
    monkeypatch.setattr(_viewer_client.psutil, "Process", Mock(return_value=launcher))
    wait = Mock(return_value=[launcher])
    monkeypatch.setattr(_viewer_client, "_wait_for_processes", wait)
    if os.name == "posix":
        monkeypatch.setattr(os, "killpg", Mock())
    else:
        monkeypatch.setattr(subprocess, "run", Mock())

    with pytest.raises(RuntimeError, match="Viewer processes did not exit"):
        viewer.close(timeout=0.25)

    assert wait.call_args_list == [call([launcher], timeout=0.25)] * 2
    launcher.kill.assert_called_once_with()
    assert viewer._pid == launcher.pid


@pytest.mark.parametrize("timeout", [0, -1, float("inf"), float("-inf"), float("nan")])
def test_close_rejects_invalid_timeout(monkeypatch: pytest.MonkeyPatch, timeout: float) -> None:
    viewer, _calls = _capturing_viewer(monkeypatch)
    with pytest.raises(ValueError, match="timeout must be finite and greater than zero"):
        viewer.close(timeout=timeout)


@pytest.mark.parametrize("gone", [False, True])
def test_wait_for_processes_excludes_terminated_descendants(gone: bool) -> None:
    process = Mock()
    process.wait.side_effect = _viewer_client.psutil.TimeoutExpired(0)
    if gone:
        process.status.side_effect = _viewer_client.psutil.NoSuchProcess(123)
    else:
        process.status.return_value = _viewer_client.psutil.STATUS_ZOMBIE
    assert _viewer_client._wait_for_processes([process], timeout=0) == []


def test_to_record_batch_single_record_batch() -> None:
    """Single RecordBatch is passed through unchanged."""
    batch = pa.record_batch({"col": [1, 2, 3]})
    result = to_record_batch(batch)
    assert result.equals(batch)


def test_to_record_batch_list_of_record_batches() -> None:
    """List of RecordBatches is concatenated into one."""
    batch1 = pa.record_batch({"col": [1, 2]})
    batch2 = pa.record_batch({"col": [3, 4]})
    result = to_record_batch([batch1, batch2])
    expected = pa.record_batch({"col": [1, 2, 3, 4]})
    assert result.equals(expected)


def test_to_record_batch_datafusion_dataframe() -> None:
    """Datafusion DataFrame is converted to a single RecordBatch."""
    ctx = datafusion.SessionContext()
    df = ctx.from_pydict({"col": [1, 2, 3]})
    result = to_record_batch(df)
    assert result.num_rows == 3
    assert result.column("col").to_pylist() == [1, 2, 3]


# TODO(andreas): Add a setter/getter round-trip test once ViewerClient has a direct time-cursor getter.
def test_set_time_uses_active_timeline_when_omitted(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, calls = _capturing_viewer(monkeypatch)
    viewer.set_time(sequence=42)

    assert calls == [(None, 42, False, None)]


def test_set_time_converts_temporal_values(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, calls = _capturing_viewer(monkeypatch)
    viewer.set_time("elapsed", duration=1.5, play=True)
    viewer.set_time("capture_time", timestamp=datetime(1970, 1, 1, tzinfo=timezone.utc))

    assert calls == [("elapsed", 1_500_000_000, True, None), ("capture_time", 0, False, None)]


def test_set_time_targets_a_named_recording(monkeypatch: pytest.MonkeyPatch) -> None:
    """Without this, only the active recording can be given a new time."""
    viewer, calls = _capturing_viewer(monkeypatch)
    viewer.set_time(sequence=7, recording="Recording:app:rec")

    assert calls == [(None, 7, False, "Recording:app:rec")]


def test_set_time_requires_exactly_one_time(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, _calls = _capturing_viewer(monkeypatch)

    with pytest.raises(ValueError, match="exactly one"):
        viewer.set_time()  # type: ignore[call-overload]

    with pytest.raises(ValueError, match="exactly one"):
        viewer.set_time(sequence=1, duration=1.0)  # type: ignore[call-overload]


def _closing_viewer(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[ViewerClient, list[tuple[str | None, list[str] | None]]]:
    calls: list[tuple[str | None, list[str] | None]] = []

    class CapturingViewerClientInternal:
        def __init__(self, _url: str) -> None:
            pass

        def close_recordings(
            self,
            target: str | None = None,
            store_ids: list[str] | None = None,
        ) -> str:
            calls.append((target, store_ids))
            return "{}"

    monkeypatch.setattr(rerun_bindings, "ViewerClientInternal", CapturingViewerClientInternal)
    return ViewerClient.connect(), calls


def test_close_recordings_selects_by_name(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, calls = _closing_viewer(monkeypatch)
    viewer.close_recordings()
    viewer.close_recordings("all")

    assert calls == [("current", None), ("all", None)]


def test_close_recordings_accepts_a_bare_store_id(monkeypatch: pytest.MonkeyPatch) -> None:
    """A store id is itself a string, so it must not be iterated character by character."""
    viewer, calls = _closing_viewer(monkeypatch)
    viewer.close_recordings("Recording:app:rec")

    assert calls == [(None, ["Recording:app:rec"])]


def test_close_recordings_accepts_several_store_ids(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, calls = _closing_viewer(monkeypatch)
    viewer.close_recordings(["Recording:a:1", "Recording:b:2"])

    assert calls == [(None, ["Recording:a:1", "Recording:b:2"])]


def test_close_recordings_of_nothing_asks_the_viewer_for_nothing(monkeypatch: pytest.MonkeyPatch) -> None:
    """An empty selection must stay empty, rather than falling back to closing everything."""
    viewer, calls = _closing_viewer(monkeypatch)
    viewer.close_recordings([])

    assert calls == [(None, [])]


def test_viewer_state_reads_an_absent_wrapper_as_none() -> None:
    """A timeline with no data has no `time_range` at all, which is not the same as a zero range."""
    state = _viewer_state_from_json({
        "recordings": [
            {
                "store_id": "Recording:app:rec",
                "timelines": [{"timeline": {"name": "frame"}, "time_type": "TIME_TYPE_SEQUENCE"}],
            }
        ]
    })

    (timeline,) = state.recordings[0].timelines
    assert timeline.start is None
    assert timeline.end is None
    assert state.recordings[0].current_time is None


def test_viewer_state_reads_a_present_empty_wrapper_as_zero() -> None:
    """
    Canonical protobuf JSON omits a scalar holding its default.

    A timeline sitting at 0 therefore arrives as `"time_range": {}`, which means zero, not absent.
    """
    state = _viewer_state_from_json({
        "recordings": [
            {
                "store_id": "Recording:app:rec",
                "timelines": [{"timeline": {"name": "frame"}, "time_type": "TIME_TYPE_SEQUENCE", "time_range": {}}],
                "current_time": {"timeline": {"name": "frame"}, "time": {}},
            }
        ]
    })

    (timeline,) = state.recordings[0].timelines
    assert timeline.start == 0
    assert timeline.end == 0
    assert state.recordings[0].current_timeline == "frame"
    assert state.recordings[0].current_time == 0


def test_viewer_state_normalizes_enum_names() -> None:
    raw = {
        "recordings": [
            {
                "store_id": "Recording:app:rec",
                "timelines": [
                    {"timeline": {"name": "log_time"}, "time_type": "TIME_TYPE_TIMESTAMP_NS"},
                    {"timeline": {"name": "since"}, "time_type": "TIME_TYPE_DURATION_NS"},
                    {"timeline": {"name": "frame"}, "time_type": "TIME_TYPE_SEQUENCE"},
                ],
            }
        ]
    }

    types = [t.time_type for t in _viewer_state_from_json(raw).recordings[0].timelines]
    assert types == ["timestamp", "duration", "sequence"]


def test_viewer_state_reads_nested_recording_and_view_fields() -> None:
    state = _viewer_state_from_json({
        "url": "rerun+http://127.0.0.1:9876/proxy",
        "catalog_url": "rerun+http://127.0.0.1:9876",
        "active_store_id": "Recording:app:rec",
        "recordings": [
            {
                "store_id": "Recording:app:rec",
                "timelines": [
                    {
                        "timeline": {"name": "frame"},
                        "time_type": "TIME_TYPE_SEQUENCE",
                        "time_range": {"start": 3, "end": 9},
                    }
                ],
            }
        ],
        "views": [
            {
                "view_id": "id",
                "class": "Spatial3D",
                "name": "world",
                "origin": "/world",
                "visible": True,
                "reports": [{"severity": "warning", "summary": "no data", "details": "nothing logged"}],
            }
        ],
    })

    assert state.active_recording == "Recording:app:rec"
    assert state.catalog_url == "rerun+http://127.0.0.1:9876"
    (timeline,) = state.recordings[0].timelines
    assert (timeline.start, timeline.end) == (3, 9)
    (view,) = state.views
    assert (view.view_class, view.name, view.origin, view.visible) == ("Spatial3D", "world", "/world", True)
    (report,) = view.reports
    assert (report.severity, report.summary, report.details) == ("warning", "no data", "nothing logged")


def test_viewer_state_of_an_empty_viewer() -> None:
    """Every field holds its default, so canonical protobuf JSON sends an empty object."""
    state = _viewer_state_from_json({})

    assert state.url == ""
    assert state.active_recording is None
    assert state.catalog_url is None
    assert state.recordings == []
    assert state.views == []
    assert state.loading == []
    assert state.viewer_version is None


def test_viewer_state_carries_the_viewer_version() -> None:
    """The version decides which API and which docs apply, so it must survive the JSON."""
    state = _viewer_state_from_json({"viewer_version": "0.38.0-alpha.1"})

    assert state.viewer_version == "0.38.0-alpha.1"


def test_viewer_state_reports_a_load_in_flight() -> None:
    """A recording whose first message has landed but whose timelines have not is still loading."""
    state = _viewer_state_from_json({
        "recordings": [{"store_id": {"application_id": {"id": "app"}, "recording_id": "episode_0"}}],
        "loading": [{"name": "/tmp/dataset", "status": "Loading /tmp/dataset…"}],
    })

    assert state.recordings[0].timelines == []
    (source,) = state.loading
    assert (source.name, source.status) == ("/tmp/dataset", "Loading /tmp/dataset…")
