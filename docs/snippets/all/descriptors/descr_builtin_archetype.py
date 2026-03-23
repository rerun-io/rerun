#!/usr/bin/env python3

from __future__ import annotations

import rerun as rr  # pip install rerun-sdk

rr.init("rerun_example_descriptors_builtin_archetype")
rr.spawn()

rr.log("data", rr.Points3D([[1, 2, 3]], radii=[0.3, 0.2, 0.1]), static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
