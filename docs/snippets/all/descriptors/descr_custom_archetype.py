#!/usr/bin/env python3

from __future__ import annotations

from typing import Any

import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk


class CustomPoints3D(rr.AsComponents):
    def __init__(self: Any, positions: npt.ArrayLike, colors: npt.ArrayLike) -> None:
        self.positions = rr.components.Position3DBatch(positions).described(
            rr.ComponentDescriptor(
                "user.CustomPosition3D",
                archetype_name="user.CustomPoints3D",
                archetype_field_name="custom_positions",
            ),
        )
        self.colors = rr.components.ColorBatch(colors).described(
            rr.ComponentDescriptor("rerun.components.Colors").with_overrides(
                archetype_name="user.CustomPoints3D",
                archetype_field_name="colors",
            )
        )

    def as_component_batches(self) -> list[rr.DescribedComponentBatch]:
        return [self.positions, self.colors]


rr.init("rerun_example_descriptors_custom_archetype")
rr.spawn()

rr.log("data", CustomPoints3D([[1, 2, 3]], [0xFF00FFFF]), static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
