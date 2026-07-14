"""Regression guard: multiple threads must be able to log to a shared FileSink inside a `with` block."""

from __future__ import annotations

import threading
from typing import TYPE_CHECKING

import rerun as rr
from rerun.experimental import RrdReader

if TYPE_CHECKING:
    from pathlib import Path

NUM_THREADS = 8
MESSAGES_PER_THREAD = 50


def _worker(rec: rr.RecordingStream, thread_id: int) -> None:
    base = thread_id * MESSAGES_PER_THREAD
    for i in range(MESSAGES_PER_THREAD):
        rec.set_time("seq", sequence=base + i)
        rec.log(f"thread_{thread_id}", rr.Scalars(float(base + i)))


def test_multithreaded_filesink_in_context_manager(tmp_path: Path) -> None:
    rrd_path = tmp_path / "multithread.rrd"

    def run() -> None:
        with rr.RecordingStream("rerun_example_multithread_filesink") as rec:
            rec.save(rrd_path)

            threads = [threading.Thread(target=_worker, args=(rec, tid)) for tid in range(NUM_THREADS)]
            for t in threads:
                t.start()
            for t in threads:
                t.join()

    run()

    # `RrdReader.store()` rejects footer-less files, so a successful call here implicitly
    # verifies the file was finalized.
    reader = RrdReader(rrd_path)
    reader.store()

    seen = dict.fromkeys(range(NUM_THREADS), 0)
    for chunk in reader.stream():
        for tid in seen:
            if chunk.entity_path == f"/thread_{tid}":
                seen[tid] += chunk.num_rows
                break

    for tid, count in seen.items():
        assert count == MESSAGES_PER_THREAD, f"thread {tid}: got {count} rows, expected {MESSAGES_PER_THREAD}"
