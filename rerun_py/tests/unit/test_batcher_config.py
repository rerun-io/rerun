from __future__ import annotations

import os
import tempfile
import time

import rerun as rr


def log_and_get_size(rec: rr.RecordingStream, path) -> int:
    """Helper function to log a message and sleep for a short duration."""
    rec.log("/data1", rr.Scalars(1))
    time.sleep(0.1)

    return os.stat(path).st_size


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

        rec.flush(blocking=True)

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
