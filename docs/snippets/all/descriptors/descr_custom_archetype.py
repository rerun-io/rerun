#!/usr/bin/env python3

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import rerun as rr  # pip install rerun-sdk

if TYPE_CHECKING:
    import numpy.typing as npt


class CustomPoints3D(rr.AsComponents):  # type: ignore[misc]
    def __init__(self: Any, positions: npt.ArrayLike, colors: npt.ArrayLike) -> None:
        self.positions = rr.components.Position3DBatch(positions).described(
            rr.ComponentDescriptor(
                "user.CustomPoints3D:custom_positions",
                archetype="user.CustomPoints3D",
                component_type="user.CustomPosition3D",
            ),
        )
        self.colors = rr.components.ColorBatch(colors).described(
            rr.ComponentDescriptor("user.CustomPoints3D:colors").with_overrides(
                archetype="user.CustomPoints3D",
                component_type=rr.components.ColorBatch._COMPONENT_TYPE,
            )
        )

    def as_component_batches(self) -> list[rr.DescribedComponentBatch]:
        return [self.positions, self.colors]


rr.init("rerun_example_descriptors_custom_archetype")
rr.spawn()

rr.log("data", CustomPoints3D([[1, 2, 3]], [0xFF00FFFF]), static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
