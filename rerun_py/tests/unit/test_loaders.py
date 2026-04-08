"""Tests for StreamingLoader and IndexedLoader."""

from __future__ import annotations

import pytest
from rerun.experimental import (
    IndexedLoader,
    RrdLoader,
    StreamingLoader,
)


@pytest.mark.local_only
def test_rrd_loader_is_streaming_loader() -> None:
    assert issubclass(RrdLoader, StreamingLoader)


@pytest.mark.local_only
def test_rrd_loader_is_indexed_loader() -> None:
    assert issubclass(RrdLoader, IndexedLoader)


@pytest.mark.local_only
def test_indexed_loader_extends_streaming_loader() -> None:
    assert issubclass(IndexedLoader, StreamingLoader)
