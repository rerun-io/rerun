"""
Parity test for `OptimizationProfile`.

Python `OptimizationProfile.{LIVE,OBJECT_STORE}` must agree with the
Rust `OptimizationProfile::{LIVE,OBJECT_STORE}` constants byte-for-byte.
"""

from __future__ import annotations

import math

import pytest
from rerun.experimental import OptimizationProfile

from rerun_bindings import _optimization_profile_values  # noqa: TID251

# Mapping: Python field -> Rust dict key. Names diverge intentionally
# (Rust mirrors `ChunkStoreConfig::chunk_max_*`; Python keeps the existing
# public `max_*` field set).
FIELD_MAP = {
    "max_bytes": "chunk_max_bytes",
    "max_rows": "chunk_max_rows",
    "max_rows_if_unsorted": "chunk_max_rows_if_unsorted",
    "extra_passes": "num_extra_passes",
    "gop_batching": "gop_batching",
    "split_size_ratio": "split_size_ratio",
}


def _python_dict(p: OptimizationProfile) -> dict[str, object]:
    return {rust_key: getattr(p, py_key) for py_key, rust_key in FIELD_MAP.items()}


@pytest.mark.parametrize(
    "name,profile",
    [
        ("LIVE", OptimizationProfile.LIVE),
        ("OBJECT_STORE", OptimizationProfile.OBJECT_STORE),
    ],
)
def test_profile_parity(name: str, profile: OptimizationProfile) -> None:
    rust = _optimization_profile_values(name)
    py = _python_dict(profile)

    # Bidirectional: a missing field on either side fails first with a clear diff.
    assert set(rust.keys()) == set(py.keys()), f"key set diverged: rust={set(rust.keys())} py={set(py.keys())}"

    for key in sorted(rust):
        rv, pv = rust[key], py[key]
        if isinstance(rv, float) or isinstance(pv, float):
            assert math.isclose(rv, pv, rel_tol=1e-9), f"{name}.{key}: rust={rv!r} py={pv!r}"  # type: ignore[arg-type]
        else:
            assert rv == pv, f"{name}.{key}: rust={rv!r} py={pv!r}"


def test_unknown_profile_name_raises() -> None:
    with pytest.raises(ValueError):
        _optimization_profile_values("BOGUS")
