"""Query and display the first 10 rows of a recording."""

import sys

import rerun as rr

path_to_rrd = sys.argv[1]

recording = rr.dataframe.load_recording(path_to_rrd)
view = recording.view(index="log_time", contents="/**")
batches = view.select()

for _ in range(10):
    row = batches.read_next_batch()
    if row is None:
        break
    # Each row is a `RecordBatch`, which can be easily passed around across different data ecosystems.
    print(row)
