"""Log a histogram as a bar chart with precomputed bins."""

import numpy as np

import rerun as rr

rr.init("rerun_example_bar_chart_histogram", spawn=True)

samples = np.random.default_rng(0).normal(size=10_000)
counts, edges = np.histogram(samples, bins=50)
rr.log(
    "histogram",
    rr.BarChart(counts, abscissa=edges[:-1], widths=np.diff(edges)),
)
