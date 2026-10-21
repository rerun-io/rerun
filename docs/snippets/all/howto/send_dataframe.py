"""Log an entire table, in a single operation."""

from __future__ import annotations

import numpy as np
import pandas as pd

import rerun as rr

rr.init("rerun_example_send_dataframe", spawn=True)

steps = np.arange(0, 64)
df = pd.DataFrame({
    "step": steps,
    "sin": np.sin(steps / 10.0),
    "cos": np.cos(steps / 10.0),
})

rr.send_dataframe(df, index="step", entity_path="/trig")
