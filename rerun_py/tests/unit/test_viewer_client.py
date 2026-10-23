from __future__ import annotations

import base64
import json
import os
import subprocess
from datetime import UTC, datetime
from typing import Any
from unittest.mock import Mock, call

import datafusion
import numpy as np
import pyarrow as pa
import pytest
from rerun._arrow import to_record_batch
from rerun.experimental import OpenResult, ViewerClient, _viewer_client
from rerun.experimental._viewer_client import _viewer_state_from_json

import rerun_bindings  # noqa: TID251


def _controlling_viewer(
    monkeypatch: pytest.MonkeyPatch,
    responses: dict[str, Any],
) -> tuple[ViewerClient, list[Any]]:
    """A viewer whose `viewer_control` answers each operation with `responses[operation]`."""
    requests: list[Any] = []

    class ControllingViewerClientInternal:
        def __init__(self, _url: str) -> None:
            pass

        def viewer_control(self, request_json: str) -> str:
            request = json.loads(request_json)
            requests.append(request)
            [operation] = request
            return json.dumps({operation: responses.get(operation, {})})

    monkeypatch.setattr(rerun_bindings, "ViewerClientInternal", ControllingViewerClientInternal)
    return ViewerClient.connect(), requests


def test_quit_without_waiting(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, _requests = _controlling_viewer(monkeypatch, {})
    viewer._pid = 123
    terminate, wait = Mock(), Mock()
    monkeypatch.setattr(_viewer_client, "_wait_for_processes", wait)
    if os.name == "posix":
        monkeypatch.setattr(os, "killpg", terminate)
    else:
        monkeypatch.setattr(subprocess, "run", terminate)

    viewer.quit(wait=False)

    terminate.assert_called_once()
    wait.assert_not_called()
    assert viewer._pid is None


def test_quit_reports_failed_teardown(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, _requests = _controlling_viewer(monkeypatch, {})
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
        viewer.quit(timeout=0.25)

    assert wait.call_args_list == [call([launcher], timeout=0.25)] * 2
    launcher.kill.assert_called_once_with()
    assert viewer._pid == launcher.pid


@pytest.mark.parametrize("timeout", [0, -1, float("inf"), float("-inf"), float("nan")])
def test_quit_rejects_invalid_timeout(monkeypatch: pytest.MonkeyPatch, timeout: float) -> None:
    viewer, _requests = _controlling_viewer(monkeypatch, {})
    with pytest.raises(ValueError, match="timeout must be finite and greater than zero"):
        viewer.quit(timeout=timeout)


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
    viewer, requests = _controlling_viewer(monkeypatch, {})
    viewer.set_time(sequence=42)

    assert requests == [{"set_time_cursor": {"time": {"time": 42}}}]


def test_set_time_converts_temporal_values(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, requests = _controlling_viewer(monkeypatch, {})
    viewer.set_time("elapsed", duration=1.5, play_state="playing")
    viewer.set_time("capture_time", timestamp=datetime(1970, 1, 1, tzinfo=UTC))
    viewer.set_time("elapsed", duration=np.timedelta64(2, "s"))

    assert requests == [
        {
            "set_time_cursor": {
                "timeline": {"name": "elapsed"},
                "time": {"time": 1_500_000_000},
                "play_state": "PLAY_STATE_PLAYING",
            }
        },
        {"set_time_cursor": {"timeline": {"name": "capture_time"}, "time": {"time": 0}}},
        {"set_time_cursor": {"timeline": {"name": "elapsed"}, "time": {"time": 2_000_000_000}}},
    ]


def test_set_time_targets_a_named_recording(monkeypatch: pytest.MonkeyPatch) -> None:
    """Without this, only the active recording can be given a new time."""
    viewer, requests = _controlling_viewer(monkeypatch, {})
    viewer.set_time(sequence=7, recording="Recording:app:rec")

    assert requests == [{"set_time_cursor": {"store_id": "Recording:app:rec", "time": {"time": 7}}}]


def test_set_time_can_follow_without_a_time(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, requests = _controlling_viewer(monkeypatch, {})
    viewer.set_time(play_state="following")

    assert requests == [{"set_time_cursor": {"play_state": "PLAY_STATE_FOLLOWING"}}]


def test_set_time_can_switch_timeline_alone(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, requests = _controlling_viewer(monkeypatch, {})
    viewer.set_time("frame")

    assert requests == [{"set_time_cursor": {"timeline": {"name": "frame"}}}]


def test_set_time_refuses_conflicting_arguments(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, _requests = _controlling_viewer(monkeypatch, {})

    with pytest.raises(ValueError, match="expects a timeline"):
        viewer.set_time()  # type: ignore[call-overload]  # ty: ignore[no-matching-overload]

    with pytest.raises(ValueError, match="at most one"):
        viewer.set_time(sequence=1, duration=1.0)  # type: ignore[call-overload]  # ty: ignore[no-matching-overload]

    with pytest.raises(ValueError, match="cannot follow"):
        viewer.set_time(sequence=1, play_state="following")  # type: ignore[call-overload]


def test_get_blueprint_picks_its_target(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, requests = _controlling_viewer(monkeypatch, {})
    viewer.get_blueprint()
    viewer.get_blueprint(store_id="Recording:app:rec")
    viewer.get_blueprint("table")

    assert requests == [
        {"get_blueprint": {}},
        {"get_blueprint": {"recording": {"store_id": "Recording:app:rec"}}},
        {"get_blueprint": {"table": {}}},
    ]

    with pytest.raises(ValueError, match="table_url"):
        viewer.get_blueprint("recording", table_url="rerun+http://localhost/entry/1")

    with pytest.raises(ValueError, match="expects"):
        viewer.set_blueprint({})


def test_close_picks_its_target(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, requests = _controlling_viewer(monkeypatch, {"close": {"closed": ["Recording:a:1"]}})

    assert viewer.close("current") == ["Recording:a:1"]
    viewer.close("all")
    viewer.close("file:///a.rrd")
    viewer.close(["a.rrd", "b.rrd"])
    viewer.close(store_ids=["Recording:a:1"])
    viewer.close(store_ids=[])

    assert requests == [
        {"close": {"current": True}},
        {"close": {"all": True}},
        {"close": {"urls": {"urls": ["file:///a.rrd"]}}},
        {"close": {"urls": {"urls": ["a.rrd", "b.rrd"]}}},
        {"close": {"store_ids": {"store_ids": ["Recording:a:1"]}}},
        {"close": {"store_ids": {"store_ids": []}}},
    ]

    with pytest.raises(ValueError, match="either"):
        viewer.close()
    with pytest.raises(ValueError, match="either"):
        viewer.close("all", store_ids=["Recording:a:1"])


def test_open_sends_one_request_per_url(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, requests = _controlling_viewer(
        monkeypatch, {"open": {"store_ids": ["Recording:a:1"], "loading": ["a.rrd"]}}
    )

    assert viewer.open("a.rrd") == OpenResult(store_ids=["Recording:a:1"], loading=["a.rrd"])
    assert viewer.open(["a.rrd", "b.rrd"], navigate=False).store_ids == ["Recording:a:1"] * 2
    assert requests == [
        {"open": {"url": "a.rrd", "navigate": True}},
        {"open": {"url": "a.rrd", "navigate": False}},
        {"open": {"url": "b.rrd", "navigate": False}},
    ]


def test_deprecated_names_still_work(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, requests = _controlling_viewer(monkeypatch, {"open": {"loading": ["a.rrd"]}})

    with pytest.deprecated_call():
        assert viewer.open_url("a.rrd") is False  # ty: ignore[deprecated]
    with pytest.deprecated_call():
        viewer.close_recordings()  # ty: ignore[deprecated]
    with pytest.deprecated_call():
        viewer.close_recordings("Recording:app:rec")  # ty: ignore[deprecated]
    with pytest.deprecated_call():
        viewer.viewer_state()  # ty: ignore[deprecated]
    with pytest.deprecated_call():
        viewer.viewer_logs()  # ty: ignore[deprecated]

    assert [next(iter(request)) for request in requests] == ["open", "close", "close", "get_state", "get_logs"]
    assert requests[2] == {"close": {"store_ids": {"store_ids": ["Recording:app:rec"]}}}


def test_take_screenshot_asks_for_png_bytes(monkeypatch: pytest.MonkeyPatch) -> None:
    png = b"\x89PNG\r\n"
    viewer, requests = _controlling_viewer(monkeypatch, {"save_screenshot": {"png": base64.b64encode(png).decode()}})

    assert viewer.take_screenshot(max_size=64) == png
    assert requests == [{"save_screenshot": {"max_size": 64}}]


def test_schema_reads_omitted_defaults(monkeypatch: pytest.MonkeyPatch) -> None:
    """Canonical protobuf JSON leaves out `false` and `0`, which must not read as missing."""
    viewer, _requests = _controlling_viewer(
        monkeypatch,
        {"get_recording_schema": {"entities": [{"entity_path": "/points", "components": [{"component": "c"}]}]}},
    )
    schema = viewer.get_recording_schema()

    assert schema.omitted_entities == 0
    assert schema.entities[0].components[0].has_static is False
    assert schema.entities[0].components[0].archetype is None


def test_viewer_control_rejects_a_mismatched_response(monkeypatch: pytest.MonkeyPatch) -> None:
    class MismatchedViewerClientInternal:
        def __init__(self, _url: str) -> None:
            pass

        def viewer_control(self, _request_json: str) -> str:
            return json.dumps({"get_location": {}})

    monkeypatch.setattr(rerun_bindings, "ViewerClientInternal", MismatchedViewerClientInternal)

    with pytest.raises(RuntimeError, match="Expected a list_commands response"):
        ViewerClient.connect().list_commands()


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
        "location": "about:recording?app_id=app&recording_id=rec",
        "catalog_url": "rerun+http://127.0.0.1:9876",
        "active_store_id": "Recording:app:rec",
        "recordings": [
            {
                "store_id": "Recording:app:rec",
                "location": "about:recording?app_id=app&recording_id=rec",
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

    assert state.location == "about:recording?app_id=app&recording_id=rec"
    assert state.recordings[0].location == "about:recording?app_id=app&recording_id=rec"
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

    assert state.location == ""
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
