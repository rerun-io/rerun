from __future__ import annotations

from rerun.experimental import OptimizationSettings


def test_optimization_settings_defaults() -> None:
    """OptimizationSettings() mirrors `rerun rrd compact` defaults."""
    s = OptimizationSettings()
    # Threshold fields default to None — resolved to ChunkStoreConfig::DEFAULT by Rust.
    assert s.max_bytes is None
    assert s.max_rows is None
    assert s.max_rows_if_unsorted is None
    assert s.extra_passes == 50
    assert s.gop_batching is True
    assert s.split_size_ratio is None


def test_optimization_settings_custom() -> None:
    """OptimizationSettings fields are individually overridable."""
    s = OptimizationSettings(max_rows=100, extra_passes=3, gop_batching=False, split_size_ratio=10.0)
    assert s.max_rows == 100
    assert s.extra_passes == 3
    assert s.gop_batching is False
    assert s.split_size_ratio == 10.0
    assert s.max_bytes is None  # unchanged from default


def test_optimization_settings_equality() -> None:
    """Dataclass-derived equality: field-by-field comparison."""
    assert OptimizationSettings() == OptimizationSettings()
    assert OptimizationSettings(extra_passes=10) == OptimizationSettings(extra_passes=10)
    assert OptimizationSettings() != OptimizationSettings(gop_batching=False)
