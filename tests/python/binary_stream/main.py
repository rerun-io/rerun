"""
Test showing that a binary stream as a valid RRD files.

After running:
```bash
rerun *.rrd
```
"""

from __future__ import annotations

import queue
import threading
import time
from typing import Any, Iterator

import rerun as rr
import rerun.blueprint as rrb


@rr.thread_local_stream("rerun_example_memory_drain")
def job(name: str) -> Iterator[tuple[str, int, bytes]]:
    stream = rr.binary_stream()

    blueprint = rrb.Blueprint(rrb.TextLogView(name="My Logs", origin="test"))

    rr.send_blueprint(blueprint)

    for i in range(5):
        time.sleep(0.2)
        rr.log("test", rr.TextLog(f"Job {name} Message {i}"))

        print(f"YIELD {name} {i}")
        yield (name, i, stream.read())


def queue_results(generator: Iterator[Any], out_queue: queue.Queue) -> None:
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

        with open(f"output_{name}.rrd", "a+b") as f:
            f.write(data)
