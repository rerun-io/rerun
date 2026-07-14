from __future__ import annotations

import dataclasses

from rerun.experimental import OptimizationProfile


def test_optimization_profile_custom() -> None:
    """OptimizationProfile fields are individually overridable."""
    p = OptimizationProfile(max_rows=100, extra_passes=3, gop_batching=False, split_size_ratio=10.0)
    assert p.max_rows == 100
    assert p.extra_passes == 3
    assert p.gop_batching is False
    assert p.split_size_ratio == 10.0
    assert p.max_bytes is None  # unchanged from default


def test_optimization_profile_equality() -> None:
    """Dataclass-derived equality: field-by-field comparison."""
    assert OptimizationProfile() == OptimizationProfile()
    assert OptimizationProfile(extra_passes=10) == OptimizationProfile(extra_passes=10)
    assert OptimizationProfile() != OptimizationProfile(gop_batching=False)


def test_replace_works() -> None:
    """`dataclasses.replace` produces a new profile with overridden fields."""
    derived = dataclasses.replace(OptimizationProfile.OBJECT_STORE, gop_batching=False)
    assert derived.gop_batching is False
    # Other fields preserved from OBJECT_STORE
    assert derived.max_bytes == OptimizationProfile.OBJECT_STORE.max_bytes
    assert derived.max_rows == OptimizationProfile.OBJECT_STORE.max_rows
    # Original is untouched
    assert OptimizationProfile.OBJECT_STORE.gop_batching is True
