#!/usr/bin/env python3

from __future__ import annotations

import rerun as rr  # pip install rerun-sdk

rr.init("rerun_example_descriptors_custom_component")
rr.spawn()

positions = rr.components.Position3DBatch([1, 2, 3]).described(
    rr.ComponentDescriptor(
        "user.CustomArchetype:custom_positions",
        archetype="user.CustomArchetype",
        component_type="user.CustomPosition3D",
    ),
)
rr.log("data", [positions], static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
