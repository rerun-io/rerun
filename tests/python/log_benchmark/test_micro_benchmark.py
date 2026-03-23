"""Per-call overhead micro-benchmarks for the rr.log() pipeline.

See README.md for usage.
"""

from __future__ import annotations

from typing import Any

import numpy as np
import pytest

import rerun as rr
import rerun_bindings as bindings
from rerun._log import _log_components

ARCHETYPE_CASES = [
    pytest.param(lambda: rr.Scalars(42.0), id="Scalars"),
    pytest.param(
        lambda: rr.Points3D([[1, 2, 3]], colors=[0xFF0000FF], radii=[0.1]),
        id="Points3D",
    ),
    pytest.param(
        lambda: rr.Transform3D(translation=[1, 2, 3], mat3x3=np.eye(3, dtype=np.float32)),
        id="Transform3D",
    ),
    pytest.param(
        lambda: rr.Boxes3D(half_sizes=[[1, 2, 3]], colors=[0xFF0000FF]),
        id="Boxes3D",
    ),
]


def _init() -> None:
    """Common setup: init rerun + memory recording."""
    rr.init("rerun_example_micro_benchmark", spawn=False)
    rr.memory_recording()


@pytest.mark.parametrize("make_archetype", ARCHETYPE_CASES)
def test_bench_micro_construct(benchmark: Any, make_archetype: Any) -> None:
    _init()
    benchmark(make_archetype)


@pytest.mark.parametrize("make_archetype", ARCHETYPE_CASES)
def test_bench_micro_as_component_batches(benchmark: Any, make_archetype: Any) -> None:
    _init()
    archetype = make_archetype()
    benchmark(archetype.as_component_batches)


@pytest.mark.parametrize("make_archetype", ARCHETYPE_CASES)
def test_bench_micro_log_components(benchmark: Any, make_archetype: Any) -> None:
    _init()
    batches = make_archetype().as_component_batches()
    benchmark(_log_components, "test_entity", batches)


@pytest.mark.parametrize("make_archetype", ARCHETYPE_CASES)
def test_bench_micro_log_arrow_msg(benchmark: Any, make_archetype: Any) -> None:
    _init()
    batches = make_archetype().as_component_batches()
    instanced = {b.component_descriptor(): b.as_arrow_array() for b in batches if b.as_arrow_array() is not None}
    benchmark(bindings.log_arrow_msg, "test_entity", components=instanced, static_=False, recording=None)


@pytest.mark.parametrize("make_archetype", ARCHETYPE_CASES)
def test_bench_micro_log(benchmark: Any, make_archetype: Any) -> None:
    _init()
    archetype = make_archetype()
    benchmark(rr.log, "test_entity", archetype)


def test_bench_micro_set_time(benchmark: Any) -> None:
    _init()
    benchmark(rr.set_time, "frame", sequence=42)
