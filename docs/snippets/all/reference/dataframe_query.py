"""Query and display the first 10 rows of a recording."""

import sys
from pathlib import Path

import rerun as rr

path_to_rrd = Path(sys.argv[1])

with rr.server.Server(datasets={"dataset": [path_to_rrd]}) as server:
    dataset = server.client().get_dataset("dataset")
    df = dataset.reader(index="log_time")

    table = df.to_arrow_table()
    top_ten = table.slice(0, min(10, table.num_rows))
    print(top_ten)
