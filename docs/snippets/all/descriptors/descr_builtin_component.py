#!/usr/bin/env python3

from __future__ import annotations

import rerun as rr  # pip install rerun-sdk

rr.init("rerun_example_descriptors_builtin_component")
rr.spawn()

rr.log(
    "data",
    [rr.components.Position3DBatch([1, 2, 3]).described()],
    static=True,
)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
