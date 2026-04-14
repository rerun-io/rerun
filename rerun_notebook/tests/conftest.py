from __future__ import annotations

import os

# Must be set before rerun_notebook is imported — prevents Wasm file reads
# and asset server startup during module initialization.
os.environ.setdefault("RERUN_NOTEBOOK_ASSET", "https://test.invalid/widget.js")

import pytest
from rerun_notebook import Viewer


@pytest.fixture
def viewer() -> Viewer:
    """A fresh Viewer that has not yet received the 'ready' signal."""
    return Viewer(width=640, height=480)


@pytest.fixture
def ready_viewer(viewer: Viewer) -> Viewer:
    """A Viewer that has already received the 'ready' signal."""
    viewer._on_ready()
    return viewer
