from __future__ import annotations

import dataclasses

import numpy as np

MAX_INT64 = 2**63 - 1
MAX_INT32 = 2**31 - 1


@dataclasses.dataclass
class Point3DInput:
    positions: np.ndarray
    colors: np.ndarray
    radii: np.ndarray
    label: str = "some label"

    @classmethod
    def prepare(cls, seed: int, num_points: int):
        rng = np.random.default_rng(seed=seed)

        return cls(
            positions=rng.integers(0, MAX_INT64, (num_points, 3)).astype(dtype=np.float32),
            colors=rng.integers(0, MAX_INT32, num_points, dtype=np.uint32),
            radii=rng.integers(0, MAX_INT64, num_points).astype(dtype=np.float32),
        )
