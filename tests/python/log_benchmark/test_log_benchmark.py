"""Python logging benchmarks. Use `just py-bench` to run."""

from __future__ import annotations

import numpy as np
import pytest
import rerun as rr

from . import Point3DInput


def log_points3d_large_batch(data: Point3DInput):
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    rr.log(
        "large_batch",
        rr.Points3D(positions=data.positions, colors=data.colors, radii=data.radii, labels=data.label),
    )


@pytest.mark.parametrize("num_points", [50_000_000])
def test_bench_points3d_large_batch(benchmark, num_points):
    rr.init("rerun_example_benchmark_points3d_large_batch")
    data = Point3DInput.prepare(42, num_points)
    benchmark(log_points3d_large_batch, data)


def log_points3d_many_individual(data: Point3DInput):
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    for i in range(data.positions.shape[0]):
        rr.log(
            "single_point",
            rr.Points3D(positions=data.positions[i], colors=data.colors[i], radii=data.radii[i]),
        )


@pytest.mark.parametrize("num_points", [100_000])
def test_bench_points3d_many_individual(benchmark, num_points):
    rr.init("rerun_example_benchmark_points3d_many_individual")
    data = Point3DInput.prepare(1337, num_points)
    benchmark(log_points3d_many_individual, data)


def log_image(image: np.ndarray, num_log_calls):
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    for i in range(num_log_calls):
        rr.log("test_image", rr.Tensor(image))


@pytest.mark.parametrize(
    ["image_dimension", "image_channels", "num_log_calls"],
    [pytest.param(16_384, 4, 4, id="16384^2px-4channels-4calls")],
)
def test_bench_image(benchmark, image_dimension, image_channels, num_log_calls):
    rr.init("rerun_example_benchmark_image")

    image = np.zeros((image_dimension, image_dimension, image_channels), dtype=np.uint8)
    benchmark(log_image, image, num_log_calls)
