"""
Parity test for `OptimizationProfile`.

Python `OptimizationProfile.{LIVE,DATAPLATFORM}` must agree with the
Rust `OptimizationProfile::{LIVE,DATAPLATFORM}` constants byte-for-byte.
"""

from __future__ import annotations

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
        ("DATAPLATFORM", OptimizationProfile.DATAPLATFORM),
    ],
)
def test_profile_parity(name: str, profile: OptimizationProfile) -> None:
    rust = _optimization_profile_values(name)
    py = _python_dict(profile)

    # Bidirectional: a missing field on either side fails first with a clear diff.
    assert set(rust.keys()) == set(py.keys()), f"key set diverged: rust={set(rust.keys())} py={set(py.keys())}"

    for key in sorted(rust):
        # All current parity values are int/bool/None. Float comparison would
        # need tolerance; if a future field uses non-integer floats, this test
        # must switch to math.isclose. Asserted defensively below.
        rv, pv = rust[key], py[key]
        assert not (isinstance(rv, float) or isinstance(pv, float)), (
            f"{name}.{key}: float values require tolerance-aware comparison; update this test"
        )
        assert rv == pv, f"{name}.{key}: rust={rv!r} py={pv!r}"


def test_unknown_profile_name_raises() -> None:
    with pytest.raises(ValueError):
        _optimization_profile_values("BOGUS")
