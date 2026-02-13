from __future__ import annotations

import os
import tempfile
import time
from datetime import timedelta

import rerun as rr


def log_and_get_size(rec: rr.RecordingStream, path: str) -> int:
    """Helper function to log a message and sleep for a short duration."""
    rec.log("/data1", rr.Scalars(1))
    time.sleep(0.1)

    return os.stat(path).st_size


def test_getter_setter() -> None:
    config = rr.ChunkBatcherConfig(
        flush_tick=0.12,
        flush_num_bytes=123,
        flush_num_rows=456,
        chunk_max_rows_if_unsorted=789,
    )

    assert config.flush_tick == timedelta(seconds=0.12)
    assert config.flush_num_bytes == 123
    assert config.flush_num_rows == 456
    assert config.chunk_max_rows_if_unsorted == 789

    # Mypy isn't happy about this, but other linters do not complain.
    config.flush_tick = 1  # type: ignore[assignment]
    assert config.flush_tick == timedelta(seconds=1)

    # Mypy isn't happy about this, but other linters do not complain.
    config.flush_tick = 2.1  # type: ignore[assignment]
    assert config.flush_tick == timedelta(seconds=2.1)

    config.flush_tick = timedelta(seconds=3.5)
    assert config.flush_tick == timedelta(seconds=3.5)

    config.flush_num_bytes = 321
    assert config.flush_num_bytes == 321

    config.flush_num_rows = 654
    assert config.flush_num_rows == 654

    config.chunk_max_rows_if_unsorted = 987
    assert config.chunk_max_rows_if_unsorted == 987


def test_partial_overrides() -> None:
    from unittest.mock import patch

    with patch.dict(os.environ, {"RERUN_FLUSH_TICK_SECS": "42", "RERUN_FLUSH_NUM_ROWS": "666"}):
        assert "RERUN_FLUSH_TICK_SECS" in os.environ
        assert "RERUN_FLUSH_NUM_ROWS" in os.environ

        config = rr.ChunkBatcherConfig(
            flush_num_bytes=123,
            chunk_max_rows_if_unsorted=789,
        )

        assert config.flush_tick == timedelta(seconds=42)
        assert config.flush_num_bytes == 123
        assert config.flush_num_rows == 666
        assert config.chunk_max_rows_if_unsorted == 789

    assert "RERUN_FLUSH_TICK_SECS" not in os.environ
    assert "RERUN_FLUSH_NUM_ROWS" not in os.environ


def test_flush_always() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rec_path = f"{tmpdir}/rec.rrd"

        rec = rr.RecordingStream(
            "rerun_example_multi_stream",
            batcher_config=rr.ChunkBatcherConfig.ALWAYS(),
        )
        rec.save(rec_path)

        sz1 = log_and_get_size(rec, rec_path)
        sz2 = log_and_get_size(rec, rec_path)

        assert sz2 > sz1, "Expected the file size to increase"


def test_flush_never() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rec_path = f"{tmpdir}/rec.rrd"

        rec = rr.RecordingStream(
            "rerun_example_multi_stream",
            batcher_config=rr.ChunkBatcherConfig.NEVER(),
        )
        rec.save(rec_path)

        sz1 = log_and_get_size(rec, rec_path)
        sz2 = log_and_get_size(rec, rec_path)
        sz3 = log_and_get_size(rec, rec_path)

        assert sz2 == sz1, "Expected the file size to stay the same"
        assert sz3 == sz2, "Expected the file size to stay the same"

        rec.flush()

        sz4 = os.stat(rec_path).st_size

        assert sz4 > sz3, "Expected the file size to increase after explicit flush"


def test_flush_custom() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rec_path = f"{tmpdir}/rec.rrd"

        batcher_config = rr.ChunkBatcherConfig.NEVER()
        batcher_config.flush_num_rows = 3

        rec = rr.RecordingStream(
            "rerun_example_multi_stream",
            batcher_config=batcher_config,
        )
        rec.save(rec_path)

        sz1 = log_and_get_size(rec, rec_path)
        sz2 = log_and_get_size(rec, rec_path)
        sz3 = log_and_get_size(rec, rec_path)

        assert sz2 == sz1, "Expected the file size to stay the same after two logs"
        assert sz3 > sz2, "Expected the file size to increase after the third log"
