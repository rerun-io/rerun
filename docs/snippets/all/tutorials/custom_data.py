"""Shows how to implement custom archetypes and components."""

from __future__ import annotations

import argparse
from typing import Any

import numpy as np
import numpy.typing as npt
import pyarrow as pa
import rerun as rr


class ConfidenceBatch(rr.ComponentBatchMixin):  # type: ignore[misc]
    """A batch of confidence data."""

    def __init__(self: Any, confidence: npt.ArrayLike) -> None:
        self.confidence = confidence

    def as_arrow_array(self) -> pa.Array:
        """The arrow batch representing the custom component."""
        return pa.array(self.confidence, type=pa.float32())


class CustomPoints3D(rr.AsComponents):  # type: ignore[misc]
    """A custom archetype that extends Rerun's builtin `Points3D` archetype with a custom component."""

    def __init__(self: Any, positions: npt.ArrayLike, confidences: npt.ArrayLike) -> None:
        self.points3d = rr.Points3D(positions)
        self.confidences = ConfidenceBatch(confidences).described(
            rr.ComponentDescriptor(
                "user.CustomPoints3D:confidences",
                archetype="user.CustomPoints3D",
                component_type="user.Confidence",
            )
        )

    def as_component_batches(self) -> list[rr.DescribedComponentBatch]:
        return [
            *self.points3d.as_component_batches(),  # The components from Points3D
            self.confidences,  # Custom confidence data
        ]


def log_custom_data() -> None:
    lin = np.linspace(-5, 5, 3)
    z, y, x = np.meshgrid(lin, lin, lin, indexing="ij")
    point_grid = np.vstack([x.flatten(), y.flatten(), z.flatten()]).T

    rr.log(
        "left/my_confident_point_cloud",
        CustomPoints3D(
            positions=point_grid,
            confidences=[42],
        ),
    )

    rr.log(
        "right/my_polarized_point_cloud",
        CustomPoints3D(positions=point_grid, confidences=np.arange(0, len(point_grid))),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_custom_data")
    log_custom_data()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
