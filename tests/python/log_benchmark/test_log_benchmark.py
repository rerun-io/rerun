"""Python logging benchmarks. Use `pixi run py-bench` to run."""

from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt
import pytest
import rerun as rr

from . import Point3DInput


def log_points3d_large_batch(data: Point3DInput) -> None:
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    rr.log(
        "large_batch",
        rr.Points3D(positions=data.positions, colors=data.colors, radii=data.radii, labels=data.label),
    )


@pytest.mark.parametrize("num_points", [50_000_000])
def test_bench_points3d_large_batch(benchmark: Any, num_points: int) -> None:
    rr.init("rerun_example_benchmark_points3d_large_batch")
    data = Point3DInput.prepare(42, num_points)
    benchmark(log_points3d_large_batch, data)


def log_points3d_many_individual(data: Point3DInput) -> None:
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    for i in range(data.positions.shape[0]):
        rr.log(
            "single_point",
            rr.Points3D(positions=data.positions[i], colors=data.colors[i], radii=data.radii[i]),
        )


@pytest.mark.parametrize("num_points", [100_000])
def test_bench_points3d_many_individual(benchmark: Any, num_points: int) -> None:
    rr.init("rerun_example_benchmark_points3d_many_individual")
    data = Point3DInput.prepare(1337, num_points)
    benchmark(log_points3d_many_individual, data)


def log_image(image: npt.NDArray[np.uint8], num_log_calls: int) -> None:
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    for _ in range(num_log_calls):
        rr.log("test_image", rr.Tensor(image))


@pytest.mark.parametrize(
    ["image_dimension", "image_channels", "num_log_calls"],
    [pytest.param(1024, 4, 20_000, id="1024^2px-4channels-20000calls")],
)
def test_bench_image(benchmark: Any, image_dimension: int, image_channels: int, num_log_calls: int) -> None:
    rr.init("rerun_example_benchmark_image")

    image = np.zeros((image_dimension, image_dimension, image_channels), dtype=np.uint8)
    benchmark(log_image, image, num_log_calls)


def test_bench_transforms_over_time_individual(
    rand_trans: npt.NDArray[np.float32],
    rand_quats: npt.NDArray[np.float32],
    rand_scales: npt.NDArray[np.float32],
) -> None:
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    num_transforms = rand_trans.shape[0]
    for i in range(num_transforms):
        rr.set_time("frame", sequence=i)
        rr.log(
            "test_transform",
            rr.Transform3D(translation=rand_trans[i], rotation=rr.Quaternion(xyzw=rand_quats[i]), scale=rand_scales[i]),
        )


def test_bench_transforms_over_time_batched(
    rand_trans: npt.NDArray[np.float32],
    rand_quats: npt.NDArray[np.float32],
    rand_scales: npt.NDArray[np.float32],
    num_transforms_per_batch: int,
) -> None:
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    num_transforms = rand_trans.shape[0]
    num_log_calls = num_transforms // num_transforms_per_batch
    for i in range(num_log_calls):
        start = i * num_transforms_per_batch
        end = (i + 1) * num_transforms_per_batch
        times = np.arange(start, end)

        rr.send_columns(
            "test_transform",
            indexes=[rr.TimeColumn("frame", sequence=times)],
            columns=rr.Transform3D.columns(
                translation=rand_trans[start:end],
                quaternion=rand_quats[start:end],
                scale=rand_scales[start:end],
            ),
        )


@pytest.mark.parametrize(
    ["num_transforms", "num_transforms_per_batch"],
    [
        pytest.param(10_000, 1),
        pytest.param(10_000, 100),
        pytest.param(10_000, 1_000),
    ],
)
def test_bench_transforms_over_time(benchmark: Any, num_transforms: int, num_transforms_per_batch: int) -> None:
    rr.init("rerun_example_benchmark_transforms_individual")

    rand_trans = np.array(np.random.rand(num_transforms, 3), dtype=np.float32)
    rand_quats = np.array(np.random.rand(num_transforms, 4), dtype=np.float32)
    rand_scales = np.array(np.random.rand(num_transforms, 3), dtype=np.float32)

    print(rand_trans.shape)

    if num_transforms_per_batch > 1:
        benchmark(
            test_bench_transforms_over_time_batched,
            rand_trans,
            rand_quats,
            rand_scales,
            num_transforms_per_batch,
        )
    else:
        benchmark(test_bench_transforms_over_time_individual, rand_trans, rand_quats, rand_scales)
