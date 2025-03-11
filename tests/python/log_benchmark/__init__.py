from __future__ import annotations

import dataclasses

import numpy as np
import numpy.typing as npt

MAX_INT64 = 2**63 - 1
MAX_INT32 = 2**31 - 1


@dataclasses.dataclass
class Point3DInput:
    positions: npt.NDArray[np.float32]
    colors: npt.NDArray[np.uint32]
    radii: npt.NDArray[np.float32]
    label: str = "some label"

    @classmethod
    def prepare(cls, seed: int, num_points: int) -> Point3DInput:
        rng = np.random.default_rng(seed=seed)

        return cls(
            positions=rng.integers(0, MAX_INT64, (num_points, 3)).astype(dtype=np.float32),
            colors=rng.integers(0, MAX_INT32, num_points, dtype=np.uint32),
            radii=rng.integers(0, MAX_INT64, num_points).astype(dtype=np.float32),
        )
