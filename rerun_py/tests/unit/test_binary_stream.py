"""Test verifying that a binary stream works and produces valid identical RRDs."""

from __future__ import annotations

import os
import queue
import subprocess
import tempfile
import threading
import time
from typing import TYPE_CHECKING, Any

import rerun as rr
import rerun.blueprint as rrb

if TYPE_CHECKING:
    from collections.abc import Iterator


@rr.thread_local_stream("rerun_example_binary_stream")
def job(name: str) -> Iterator[tuple[str, bytes | None]]:
    stream = rr.binary_stream()

    blueprint = rrb.Blueprint(rrb.TextLogView(name="My Logs", origin="test"))

    rr.send_blueprint(blueprint)

    for i in range(100):
        time.sleep(0.01)
        rr.log("test", rr.TextLog(f"Message {i}"))

        yield (name, stream.read())


def queue_results(generator: Iterator[Any], out_queue: queue.Queue[tuple[str, bytes]]) -> None:
    for item in generator:
        out_queue.put(item)


def test_binary_stream() -> None:
    # Flush num rows must be 0 to avoid inconsistencies in the stream
    prev_flush_num_rows = os.environ.get("RERUN_FLUSH_NUM_ROWS")
    os.environ["RERUN_FLUSH_NUM_ROWS"] = "0"

    results_queue: queue.Queue[tuple[str, bytes | None]] = queue.Queue()

    threads = [
        threading.Thread(target=queue_results, args=(job("A"), results_queue)),
        threading.Thread(target=queue_results, args=(job("B"), results_queue)),
    ]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    with tempfile.TemporaryDirectory() as tmpdir:
        while not results_queue.empty():
            name, data = results_queue.get()

            assert data is not None

            # Bump this value down when we have less overhead.
            assert len(data) > 1000

            with open(f"{tmpdir}/output_{name}.rrd", "a+b") as f:
                f.write(data)

        process = subprocess.run(
            ["rerun", "rrd", "compare", f"{tmpdir}/output_A.rrd", f"{tmpdir}/output_B.rrd"],
            check=False,
            capture_output=True,
        )
        if process.returncode != 0:
            print(process.stderr.decode("utf-8"))
            raise Exception("Rerun failed")

    # Restore the previous value of RERUN_FLUSH_NUM_ROWS
    if prev_flush_num_rows is not None:
        os.environ["RERUN_FLUSH_NUM_ROWS"] = prev_flush_num_rows
    else:
        del os.environ["RERUN_FLUSH_NUM_ROWS"]
