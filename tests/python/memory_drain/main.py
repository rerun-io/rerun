"""
Test showing that memory can be drained from a memory recording as valid RRD files.

After running:
```bash
rerun *.rrd
```
"""

from __future__ import annotations

import queue
import threading
import time
from typing import TYPE_CHECKING, Any

import rerun as rr
import rerun.blueprint as rrb

if TYPE_CHECKING:
    from collections.abc import Iterator


@rr.thread_local_stream("rerun_example_memory_drain")
def job(name: str) -> Iterator[tuple[str, int, bytes]]:
    mem = rr.memory_recording()

    blueprint = rrb.Blueprint(rrb.TextLogView(name="My Logs", origin="test"))

    rr.send_blueprint(blueprint)

    for i in range(5):
        time.sleep(0.2)
        rr.log("test", rr.TextLog(f"Job {name} Message {i}"))

        print(f"YIELD {name} {i}")
        yield (name, i, mem.drain_as_bytes())


def queue_results(generator: Iterator[Any], out_queue: queue.Queue[Any]) -> None:
    for item in generator:
        out_queue.put(item)


if __name__ == "__main__":
    results_queue: queue.Queue[tuple[str, int, bytes]] = queue.Queue()

    threads = [
        threading.Thread(target=queue_results, args=(job("A"), results_queue)),
        threading.Thread(target=queue_results, args=(job("B"), results_queue)),
        threading.Thread(target=queue_results, args=(job("C"), results_queue)),
    ]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    while not results_queue.empty():
        name, i, data = results_queue.get()

        with open(f"output_{name}_{i}.rrd", "wb") as f:
            f.write(data)
