"""Use `AnyValues` and `send_column` to send entire columns of custom data to Rerun."""

from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("rerun_example_any_values_send_columns", spawn=True)

timestamps = np.arange(0, 64)

# Log two component columns, named "sin" and "cos", with the corresponding values
rr.send_columns_v2(
    "/",
    indexes=[rr.TimeSequenceColumn("step", timestamps)],
    columns=rr.AnyValues.columns(sin=np.sin(timestamps / 10.0), cos=np.cos(timestamps / 10.0)),
)
