#!/usr/bin/env python3
"""Log a scalar scalar batch."""

import numpy as np
import pyarrow as pa
import rerun as rr

rr.init("rerun_example_temporal_batch", spawn=True)

times = np.arange(0, 64, 1)
scalars = np.sin(times / 10.0)

times = pa.array(times, type=pa.int64())
scalars = pa.array(scalars, type=pa.float64())

rr.bindings.log_arrow_chunk(
    "scalars", timelines={"step": times}, components={rr.components.Scalar.component_name(): scalars}
)
