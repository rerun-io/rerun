#!/usr/bin/env python3

from __future__ import annotations

from typing import Any, Iterable

import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk


class CustomPoints3D(rr.AsComponents):
    def __init__(self: Any, positions: npt.ArrayLike, colors: npt.ArrayLike) -> None:
        self.positions = rr.components.Position3DBatch(positions).with_descriptor(
            rr.ComponentDescriptor(
                "user.CustomPosition3D",
                archetype_name="user.CustomPoints3D",
                archetype_field_name="custom_positions",
            )
        )
        self.colors = rr.components.ColorBatch(colors).or_with_descriptor_overrides(
            archetype_name="user.CustomPoints3D",
            archetype_field_name="colors",
        )

    def as_component_batches(self) -> Iterable[rr.ComponentBatchLike]:
        print([rr.IndicatorComponentBatch("user.CustomPoints3D"), self.positions, self.colors])
        return [rr.IndicatorComponentBatch("user.CustomPoints3D"), self.positions, self.colors]


rr.init("rerun_example_descriptors_custom_archetype")
rr.spawn()

rr.log("data", CustomPoints3D([[1, 2, 3]], [0xFF00FFFF]), static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
