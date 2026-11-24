"""Log a user-defined class by remapping it to a Rerun archetype."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

# region: labeled_points
import rerun as rr  # pip install rerun-sdk


@dataclass
class LabeledPoints:
    points: np.ndarray
    labels: list[str]

    def as_component_batches(self) -> list[rr.DescribedComponentBatch]:
        return rr.Points3D(
            positions=self.points, labels=self.labels
        ).as_component_batches()


# endregion: labeled_points


def my_points_classifier() -> LabeledPoints:
    return LabeledPoints(
        points=np.array([[0, 0, 0], [1, 0, 0], [0, 1, 0]], dtype=np.float32),
        labels=["origin", "x", "y"],
    )


rr.init("rerun_example_remap_to_archetype", spawn=True)

# region: log
# Somewhere deep in your code
classified = my_points_classifier()
rr.log("points/classified", classified)
# endregion: log
