#!/usr/bin/env python3

from __future__ import annotations

from typing import Any, Iterable

import numpy.typing as npt
import pyarrow as pa
import rerun as rr  # pip install rerun-sdk


class CustomPosition3DBatch(rr.ComponentBatchLike):
    def __init__(self: Any, positions: npt.ArrayLike) -> None:
        self.position = rr.components.Position3DBatch(positions)

    def component_descriptor(self) -> rr.ComponentDescriptor:
        return rr.ComponentDescriptor(
            "user.CustomPosition3D",
            archetype_name="user.CustomPoints3D",
            archetype_field_name="custom_positions",
        )

    def as_arrow_array(self) -> pa.Array:
        return self.position.as_arrow_array()


class CustomPoints3D(rr.AsComponents):
    def __init__(self: Any, positions: npt.ArrayLike, colors: npt.ArrayLike) -> None:
        self.positions = CustomPosition3DBatch(positions)
        self.colors = rr.components.ColorBatch(colors)

    def as_component_batches(self) -> Iterable[rr.ComponentBatchLike]:
        return (
            [rr.IndicatorComponentBatch("user.CustomPoints3D")]
            + [
                rr.DescribedComponentBatch(
                    self.positions,
                    self.positions.component_descriptor().or_with_overrides(
                        archetype_name="user.CustomPoints3D", archetype_field_name="custom_positions"
                    ),
                )
            ]
            + [
                rr.DescribedComponentBatch(
                    self.colors,
                    self.colors.component_descriptor().or_with_overrides(
                        archetype_name="user.CustomPoints3D", archetype_field_name="colors"
                    ),
                )
            ]
        )


rr.init("rerun_example_descriptors_custom_archetype")
rr.spawn()

rr.log("data", CustomPoints3D([[1, 2, 3]], [0xFF00FFFF]), static=True)

# The tags are indirectly checked by the Rust version (have a look over there for more info).
