"""
Test showing that memory can be drained from a memory recording as valid RRD files.

After running:
```bash
rerun *.rrd
```
"""

from __future__ import annotations

import rerun as rr
import rerun.blueprint as rrb


def main() -> None:
    with rr.new_recording("rerun_example_memory_drain"):
        mem = rr.memory_recording()

        blueprint = rrb.Blueprint(rrb.TextLogView(name="My Logs", origin="test"))

        rr.send_blueprint(blueprint)

        for i in range(5):
            rr.log("test", rr.TextLog(f"Message {i}"))

            with open(f"output_{i}.rrd", "wb") as f:
                f.write(mem.drain_as_bytes())


if __name__ == "__main__":
    main()
