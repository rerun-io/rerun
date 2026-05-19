"""Tests for the high-level send_table orchestration in rerun.notebook.Viewer."""

from __future__ import annotations

from contextlib import ExitStack
from unittest.mock import MagicMock, PropertyMock, patch

import pyarrow as pa
import pytest


@pytest.fixture
def mocked_viewer() -> tuple[MagicMock, MagicMock]:
    """Create a high-level rerun.notebook.Viewer with all heavy deps mocked out.

    Yields the Viewer along with the mock for its low-level rerun_notebook.Viewer
    instance so tests can inspect calls made to it. The mocks remain active for
    the entire test body — in particular `_flush_ui_events` must stay patched,
    since `jupyter_ui_poll` requires a real kernel.
    """
    with ExitStack() as stack:
        stack.enter_context(patch("rerun.notebook._ErrorWidget"))
        MockViewer = stack.enter_context(patch("rerun.notebook._Viewer"))
        stack.enter_context(patch("rerun.notebook._HTML"))
        stack.enter_context(patch("rerun.notebook._VBox"))
        stack.enter_context(patch("rerun.notebook._flush_ui_events"))
        mock_bindings = stack.enter_context(patch("rerun.notebook.bindings"))
        mock_bindings.get_credentials.return_value = None

        from rerun.notebook import Viewer

        viewer = Viewer(width=640, height=480)
        low_level = MockViewer.return_value
        # Reset so __init__ calls (like _on_raw_event) don't pollute assertions.
        low_level.reset_mock()
        yield viewer, low_level


SAMPLE_BATCH = pa.RecordBatch.from_pydict({"x": [1, 2, 3]})


def test_send_table_sends_bytes(mocked_viewer: tuple[MagicMock, MagicMock]) -> None:
    viewer, low_level = mocked_viewer
    viewer.send_table("t", SAMPLE_BATCH)

    args, _kwargs = low_level.send_table.call_args
    assert isinstance(args[0], bytes)
    assert len(args[0]) > 0


def test_send_table_does_not_block_on_viewer_ready(mocked_viewer: tuple[MagicMock, MagicMock]) -> None:
    # The spinner now renders as a standalone output widget, so send_table should
    # not wait for the Viewer Wasm to load.
    viewer, low_level = mocked_viewer
    viewer.send_table("t", SAMPLE_BATCH)

    low_level.block_until_ready.assert_not_called()


def test_send_table_sets_then_clears_loading_widget(mocked_viewer: tuple[MagicMock, MagicMock]) -> None:
    # The loading widget value should land on "" across the call so the indicator
    # disappears once the data has been sent.
    viewer, _ = mocked_viewer
    viewer.send_table("my_table", SAMPLE_BATCH)

    assert viewer._loading_widget.value == ""


def test_send_table_loading_html_mentions_table_id(mocked_viewer: tuple[MagicMock, MagicMock]) -> None:
    # Use PropertyMock to record every get/set of the loading widget's `value`
    # attribute, then filter to the setter calls (which carry the new value).
    viewer, _ = mocked_viewer
    value_mock = PropertyMock(return_value="")
    type(viewer._loading_widget).value = value_mock

    viewer.send_table("my_table", SAMPLE_BATCH)

    assigned = [call.args[0] for call in value_mock.call_args_list if call.args]
    assert any("my_table" in v for v in assigned)
    assert assigned[-1] == ""


def test_display_groups_loading_and_viewer_in_vbox(mocked_viewer: tuple[MagicMock, MagicMock]) -> None:
    # The loading indicator must share a layout container with the Viewer so its
    # height is tied to the viewer's layout box rather than living in its own
    # notebook output cell.
    viewer, low_level = mocked_viewer

    with patch("rerun.notebook._VBox") as mock_vbox, patch("IPython.display.display"):
        viewer.display()

    (children,), _kwargs = mock_vbox.call_args
    assert children == [viewer._loading_widget, low_level]


def test_send_table_clears_loading_widget_on_error(mocked_viewer: tuple[MagicMock, MagicMock]) -> None:
    # If the table send raises, the spinner must still be cleared so the user
    # isn't left staring at a stuck progress indicator.
    viewer, low_level = mocked_viewer
    low_level.send_table.side_effect = RuntimeError("boom")

    with pytest.raises(RuntimeError):
        viewer.send_table("t", SAMPLE_BATCH)

    assert viewer._loading_widget.value == ""
