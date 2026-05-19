"""Tests for the low-level rerun_notebook.Viewer widget."""

from __future__ import annotations

from unittest.mock import patch

import anywidget
from rerun_notebook import Viewer

# --- Event queue gating ---


def test_starts_not_ready(viewer: Viewer) -> None:
    assert viewer._ready is False
    assert viewer._event_queue == []


def test_send_queues_before_ready(viewer: Viewer) -> None:
    viewer.send({"type": "test"})
    assert len(viewer._event_queue) == 1
    assert viewer._event_queue[0] == ({"type": "test"}, None)


def test_send_queues_with_buffers(viewer: Viewer) -> None:
    viewer.send({"type": "rrd"}, buffers=[b"some_data"])
    assert viewer._event_queue[0] == ({"type": "rrd"}, [b"some_data"])


def test_on_ready_flushes_queue(viewer: Viewer) -> None:
    viewer.send({"type": "a"})
    viewer.send({"type": "b"}, buffers=[b"data"])

    with patch.object(anywidget.AnyWidget, "send") as mock_send:
        viewer._on_ready()

    assert viewer._ready is True
    assert viewer._event_queue == []
    assert mock_send.call_count == 2
    mock_send.assert_any_call({"type": "a"}, None)
    mock_send.assert_any_call({"type": "b"}, [b"data"])


def test_send_after_ready(ready_viewer: Viewer) -> None:
    with patch.object(anywidget.AnyWidget, "send") as mock_send:
        ready_viewer.send({"type": "test"}, buffers=[b"x"])

    mock_send.assert_called_once_with({"type": "test"}, [b"x"])


# --- Message shapes ---


def test_send_rrd_shape(ready_viewer: Viewer) -> None:
    with patch.object(anywidget.AnyWidget, "send") as mock_send:
        ready_viewer.send_rrd(b"rrd_bytes")

    mock_send.assert_called_once_with({"type": "rrd"}, [b"rrd_bytes"])


def test_send_table_shape(ready_viewer: Viewer) -> None:
    with patch.object(anywidget.AnyWidget, "send") as mock_send:
        ready_viewer.send_table(b"table_bytes")

    mock_send.assert_called_once_with({"type": "table"}, [b"table_bytes"])


# --- Instance isolation ---


def test_instance_isolation() -> None:
    v1 = Viewer(width=100, height=100)
    v2 = Viewer(width=100, height=100)

    v1.send({"type": "a"})

    assert len(v1._event_queue) == 1
    assert len(v2._event_queue) == 0

    v1._on_ready()
    assert v1._ready is True
    assert v2._ready is False


# --- Message callbacks ---


def test_ready_message_callback(viewer: Viewer) -> None:
    assert viewer._ready is False

    # Simulate the kernel delivering a "ready" string message.
    # The handle_msg callback registered in __init__ should call _on_ready.
    viewer._on_ready()

    assert viewer._ready is True


def test_raw_event_callback(viewer: Viewer) -> None:
    received: list[str] = []
    viewer._on_raw_event(received.append)

    # Deliver a non-"ready" string — should go to raw event callbacks.
    for callback in viewer._raw_event_callbacks:
        callback("some_event_json")

    assert received == ["some_event_json"]
