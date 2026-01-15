"""
Shows how to configure micro-batching directly from code.

Check out <https://rerun.io/docs/reference/sdk/micro-batching> for more information.
"""

from datetime import timedelta
import rerun as rr

# Equivalent to configuring the following environment:
# * RERUN_FLUSH_NUM_BYTES=<+inf>
# * RERUN_FLUSH_NUM_ROWS=10
# * RERUN_FLUSH_TICK_SECS=10
config = rr.ChunkBatcherConfig(
    flush_num_bytes=2**63,
    flush_num_rows=10,
    flush_tick=timedelta(seconds=10),
)

rec = rr.RecordingStream("rerun_example_micro_batching", batcher_config=config)
rec.spawn()

# These 10 log calls are guaranteed be batched together, and end up in the same chunk.
for i in range(10):
    rec.log("logs", rr.TextLog(f"log #{i}"))
