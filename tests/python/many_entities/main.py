from __future__ import annotations

import math

import rerun as rr

rr.init("rerun_example_benchmark_many_entities", spawn=True)

for i in range(1000):
    f = i * 0.1
    rr.log("i" + str(i), rr.Points3D([math.sin(f), f, math.cos(f)]))
