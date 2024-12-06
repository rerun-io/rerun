#!/usr/bin/env python3

from __future__ import annotations

from typing import Any

import numpy.typing as npt
import pyarrow as pa
import rerun as rr  # pip install rerun-sdk


class CustomPosition3DBatch(rr.ComponentBatchLike):
    def __init__(self: Any, positions: npt.ArrayLike) -> None:
        self.position = rr.components.Position3DBatch(positions)

    def component_descriptor(self) -> rr.ComponentDescriptor:
        return rr.ComponentDescriptor(
            "user.CustomPosition3D",
            archetype_name="user.CustomArchetype",
            archetype_field_name="custom_positions",
        )

    def as_arrow_array(self) -> pa.Array:
        return self.position.as_arrow_array()


rr.init("rerun_example_descriptors_custom_component")
rr.spawn()

rr.log("data", [CustomPosition3DBatch([[1, 2, 3]])], static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
