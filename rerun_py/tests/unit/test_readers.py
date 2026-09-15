"""Tests for StreamingReader and IndexedReader."""

from __future__ import annotations

from rerun.chunk import IndexedReader, RrdReader, StreamingReader


def test_rrd_reader_is_streaming_reader() -> None:
    assert issubclass(RrdReader, StreamingReader)


def test_rrd_reader_is_indexed_reader() -> None:
    assert issubclass(RrdReader, IndexedReader)


def test_indexed_reader_extends_streaming_reader() -> None:
    assert issubclass(IndexedReader, StreamingReader)
