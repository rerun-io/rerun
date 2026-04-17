"""Tests for StreamingReader and IndexedReader."""

from __future__ import annotations

import pytest
from rerun.experimental import (
    IndexedReader,
    RrdReader,
    StreamingReader,
)


@pytest.mark.local_only
def test_rrd_reader_is_streaming_reader() -> None:
    assert issubclass(RrdReader, StreamingReader)


@pytest.mark.local_only
def test_rrd_reader_is_indexed_reader() -> None:
    assert issubclass(RrdReader, IndexedReader)


@pytest.mark.local_only
def test_indexed_reader_extends_streaming_reader() -> None:
    assert issubclass(IndexedReader, StreamingReader)
