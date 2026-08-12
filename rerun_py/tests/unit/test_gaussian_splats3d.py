from __future__ import annotations

import numpy as np
import pytest
import rerun as rr


def test_spherical_harmonics3rgb_accepts_coefficient_major_shapes() -> None:
    """`(15, 3)`, `(45,)`, and batches thereof are all coefficient-major."""
    for shape in [(15, 3), (45,), (4, 15, 3), (4, 45)]:
        values = np.arange(int(np.prod(shape)), dtype=np.float16).reshape(shape)
        batch = rr.datatypes.SphericalHarmonics3RgbBatch(values)
        assert len(batch.as_arrow_array()) == max(int(np.prod(shape)) // 45, 1)


def test_spherical_harmonics3rgb_rejects_channel_major() -> None:
    """A channel-major `(3, 15)` array has 45 elements too — it must not be silently transposed."""
    rr.set_strict_mode(True)

    with pytest.raises(ValueError):
        rr.datatypes.SphericalHarmonics3RgbBatch(np.zeros((3, 15), dtype=np.float16))


def test_spherical_harmonics3rgb_rejects_unpadded_lower_degree() -> None:
    """Degree 2 is 8 coefficients per channel; it must be zero-padded to 15, not regrouped."""
    rr.set_strict_mode(True)

    with pytest.raises(ValueError):
        rr.datatypes.SphericalHarmonics3RgbBatch(np.zeros((45, 8, 3), dtype=np.float16))


def test_gaussian_splats3d_roundtrip() -> None:
    arch = rr.GaussianSplats3D(
        [(0.0, 0.0, 0.0), (2.0, 0.0, 0.0)],
        scales=[(1.0, 0.5, 0.25), (0.5, 1.0, 0.5)],
        colors=[0xFF0000FF, 0x00FF00FF],
        sh_coefficients=[np.full((15, 3), 0.5), np.zeros((15, 3))],
    )

    batches = {batch.component_descriptor().component: batch for batch in arch.as_component_batches()}
    assert len(batches["GaussianSplats3D:centers"].as_arrow_array()) == 2
    assert len(batches["GaussianSplats3D:sh_coefficients"].as_arrow_array()) == 2
