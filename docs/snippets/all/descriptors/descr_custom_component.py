#!/usr/bin/env python3

from __future__ import annotations

import rerun as rr  # pip install rerun-sdk

rr.init("rerun_example_descriptors_custom_component")
rr.spawn()

positions = rr.components.Position3DBatch([1, 2, 3]).with_descriptor(
    rr.ComponentDescriptor(
        "user.CustomPosition3D",
        archetype_name="user.CustomArchetype",
        archetype_field_name="custom_positions",
    )
)
rr.log("data", [positions], static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
