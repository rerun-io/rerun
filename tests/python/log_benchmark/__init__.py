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


@dataclasses.dataclass
class Transform3DInput:
    """Input data for Transform3D benchmark with translation and mat3x3."""

    translations: npt.NDArray[np.float32]  # Shape: (num_time_steps, num_entities, 3)
    mat3x3s: npt.NDArray[np.float32]  # Shape: (num_time_steps, num_entities, 3, 3)
    num_entities: int
    num_time_steps: int

    @classmethod
    def prepare(cls, seed: int, num_entities: int, num_time_steps: int) -> Transform3DInput:
        rng = np.random.default_rng(seed=seed)

        # Generate translations in range [0, 10)
        translations = rng.random((num_time_steps, num_entities, 3), dtype=np.float32) * 10.0

        # Generate mat3x3 values in range [-1, 1)
        mat3x3s = rng.random((num_time_steps, num_entities, 3, 3), dtype=np.float32) * 2.0 - 1.0

        return cls(
            translations=translations,
            mat3x3s=mat3x3s,
            num_entities=num_entities,
            num_time_steps=num_time_steps,
        )
