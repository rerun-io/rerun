"""Tests for the marimo code paths in rerun_notebook and rerun.notebook."""

from __future__ import annotations

import sys
from contextlib import ExitStack
from typing import TYPE_CHECKING
from unittest.mock import MagicMock, patch

import pyarrow as pa
import pytest
from rerun_notebook import Viewer as LowLevelViewer
from rerun_notebook import running_in_marimo

if TYPE_CHECKING:
    from collections.abc import Iterator

    from rerun.notebook import Viewer


@pytest.fixture
def fake_marimo() -> Iterator[MagicMock]:
    """Install a stand-in `marimo` module that reports a running notebook kernel."""
    mo = MagicMock()
    mo.running_in_notebook.return_value = True
    with patch.dict(sys.modules, {"marimo": mo}):
        yield mo


@pytest.fixture
def mocked_viewer(fake_marimo: MagicMock) -> Iterator[tuple[Viewer, MagicMock]]:
    with ExitStack() as stack:
        stack.enter_context(patch("rerun.notebook._ErrorWidget"))
        MockViewer = stack.enter_context(patch("rerun.notebook._Viewer"))
        stack.enter_context(patch("rerun.notebook._HTML"))
        mock_bindings = stack.enter_context(patch("rerun.notebook.bindings"))
        mock_bindings.get_credentials.return_value = None

        import rerun.notebook

        viewer = rerun.notebook.Viewer(width=640, height=480)
        low_level = MockViewer.return_value
        low_level.reset_mock()
        yield viewer, low_level


def test_not_in_marimo_without_marimo_import() -> None:
    # Jupyter users usually don't have marimo installed, so detection must not import it.
    with patch.dict(sys.modules):
        sys.modules.pop("marimo", None)
        assert running_in_marimo() is False
        assert "marimo" not in sys.modules


def test_block_until_ready_does_not_poll_jupyter(fake_marimo: MagicMock) -> None:
    viewer = LowLevelViewer(width=640, height=480)
    with patch("rerun_notebook.jupyter_ui_poll.ui_events") as ui_events:
        viewer.block_until_ready(timeout=10.0)
    ui_events.assert_not_called()


def test_marimo_renders_only_the_anywidget(fake_marimo: MagicMock, mocked_viewer: tuple[Viewer, MagicMock]) -> None:
    # marimo shows an error banner for built-in ipywidgets such as the `VBox` used in Jupyter.
    viewer, low_level = mocked_viewer

    assert viewer._display_() is low_level

    with patch("rerun.notebook._VBox") as mock_vbox:
        viewer.display()
    fake_marimo.output.append.assert_called_once_with(low_level)
    mock_vbox.assert_not_called()


def test_send_table_does_not_poll_jupyter(mocked_viewer: tuple[Viewer, MagicMock]) -> None:
    viewer, low_level = mocked_viewer

    with patch("rerun.notebook._flush_ui_events") as flush:
        viewer.send_table("my_table", pa.RecordBatch.from_pydict({"x": [1, 2, 3]}))

    flush.assert_not_called()
    low_level.send_table.assert_called_once()
