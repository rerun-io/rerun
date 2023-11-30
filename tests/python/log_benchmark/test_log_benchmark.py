from __future__ import annotations

import numpy as np
import rerun as rr

from . import Point3DInput


def log_points3d_large_batch(data: Point3DInput):
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    rr.log(
        "large_batch",
        rr.Points3D(positions=data.positions, colors=data.colors, radii=data.radii, labels=data.label),
    )


def test_bench_points3d_large_batch(benchmark):
    rr.init("rerun_example_benchmark_points3d_large_batch")
    data = Point3DInput.prepare(42, 50_000_000)
    benchmark(log_points3d_large_batch, data)


def log_points3d_many_individual(data: Point3DInput):
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    for i in range(data.positions.shape[0]):
        rr.log(
            "single_point",
            rr.Points3D(positions=data.positions[i], colors=data.colors[i], radii=data.radii[i]),
        )


def test_bench_points3d_many_individual(benchmark):
    rr.init("rerun_example_benchmark_points3d_many_individual")
    data = Point3DInput.prepare(1337, 100_000)
    benchmark(log_points3d_many_individual, data)


IMAGE_DIMENSION = 16_384
IMAGE_CHANNELS = 4
NUM_LOG_CALLS = 4


def log_image(image: np.ndarray):
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    for i in range(NUM_LOG_CALLS):
        rr.log("test_image", rr.Tensor(image))


def test_bench_image(benchmark):
    rr.init("rerun_example_benchmark_image")

    image = np.zeros((IMAGE_DIMENSION, IMAGE_DIMENSION, IMAGE_CHANNELS), dtype=np.uint8)
    benchmark(log_image, image)
