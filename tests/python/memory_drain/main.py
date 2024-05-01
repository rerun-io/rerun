"""
Test showing that memory can be drained from a memory recording as valid RRD files.

After running:
```bash
rerun *.rrd
```
"""

from __future__ import annotations

import threading
import time
import uuid

import rerun as rr
import rerun.blueprint as rrb


def job(name: str) -> None:
    with rr.new_recording("rerun_example_memory_drain", recording_id=uuid.uuid4()):
        mem = rr.memory_recording()

        blueprint = rrb.Blueprint(rrb.TextLogView(name="My Logs", origin="test"))

        rr.send_blueprint(blueprint)

        for i in range(5):
            time.sleep(0.2)
            rr.log("test", rr.TextLog(f"Job {name} Message {i}"))

            with open(f"output_{name}_{i}.rrd", "wb") as f:
                f.write(mem.drain_as_bytes())


if __name__ == "__main__":
    threading.Thread(target=job, args=("A",)).start()
    threading.Thread(target=job, args=("B",)).start()
    threading.Thread(target=job, args=("C",)).start()
