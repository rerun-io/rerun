"""
Python logging benchmarks.

Running benchmarks with pytest-benchmark
----------------------------------------
From the `rerun/` directory:

    # Run all benchmarks:
    pixi run py-bench

    # Run a specific benchmark:
    pixi run py-bench -- -k transform3d_translation_mat3x3

Running standalone (for profiling)
----------------------------------
From the `rerun/` directory, first enter the pixi shell:

    pixi shell

Then run the benchmark:

    # Run directly:
    uvpy -m tests.python.log_benchmark.test_log_benchmark transform3d

    # With options:
    uvpy -m tests.python.log_benchmark.test_log_benchmark transform3d --num-entities 10 --num-time-steps 10000 --static

    # Connect to a running Rerun viewer (start `rerun` first):
    uvpy -m tests.python.log_benchmark.test_log_benchmark transform3d --connect

    # With py-spy flamegraph (on Linux, add --native for native stack traces):
    sudo PYTHONPATH=rerun_py/rerun_sdk:rerun_py py-spy record -o flamegraph.svg -- .venv/bin/python -m tests.python.log_benchmark.test_log_benchmark transform3d

    # Then open flamegraph.svg in a browser
"""

from __future__ import annotations

import argparse
import time
from typing import Any

import numpy as np
import numpy.typing as npt
import pytest
import rerun as rr

from . import Point3DInput, Transform3DInput


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


def log_transform3d_translation_mat3x3(data: Transform3DInput, static: bool) -> None:
    """Log Transform3D with translation and mat3x3 for each entity at each time step."""
    # create a new, empty memory sink for the current recording
    rr.memory_recording()

    start = time.perf_counter()

    for time_index in range(data.num_time_steps):
        for entity_index in range(data.num_entities):
            entity_path = f"transform_{entity_index}"
            transform = rr.Transform3D(
                translation=data.translations[time_index, entity_index].tolist(),
                mat3x3=np.array(data.mat3x3s[time_index, entity_index], dtype=np.float32),
            )

            if static:
                rr.log(entity_path, transform, static=True)
            else:
                rr.set_time("frame", sequence=time_index)
                rr.set_time("sim_time", duration=time_index * 0.01)
                rr.log(entity_path, transform)

    elapsed = time.perf_counter() - start
    total_log_calls = data.num_entities * data.num_time_steps
    transforms_per_second = total_log_calls / elapsed
    print(f"Logged {total_log_calls} transforms in {elapsed:.2f}s ({transforms_per_second:.0f} transforms/second)")


@pytest.mark.parametrize(
    ["num_entities", "num_time_steps", "static"],
    [
        pytest.param(10, 10_000, False, id="10entities-10000steps-temporal"),
        pytest.param(10, 10_000, True, id="10entities-10000steps-static"),
    ],
)
def test_bench_transform3d_translation_mat3x3(
    benchmark: Any, num_entities: int, num_time_steps: int, static: bool
) -> None:
    rr.init("rerun_example_benchmark_transform3d_translation_mat3x3")
    data = Transform3DInput.prepare(42, num_entities, num_time_steps)
    benchmark(log_transform3d_translation_mat3x3, data, static)


# -----------------------------------------------------------------------------
# Standalone execution (for profiling with py-spy, etc.)
# -----------------------------------------------------------------------------

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run logging benchmarks standalone (useful for profiling)")
    parser.add_argument(
        "benchmark",
        choices=["transform3d"],
        help="Which benchmark to run",
    )
    parser.add_argument("--num-entities", type=int, default=10, help="Number of entities")
    parser.add_argument("--num-time-steps", type=int, default=1_000, help="Number of time steps")
    parser.add_argument("--static", action="store_true", help="Log as static data")
    parser.add_argument(
        "--connect",
        action="store_true",
        help="Connect to a running Rerun viewer via gRPC instead of using memory recording",
    )
    args = parser.parse_args()

    if args.benchmark == "transform3d":
        total_log_calls = args.num_entities * args.num_time_steps
        print(
            f"Preparing {total_log_calls} transforms ({args.num_entities} entities x {args.num_time_steps} time steps)…"
        )
        rr.init("rerun_example_benchmark_transform3d_translation_mat3x3", spawn=False)
        if args.connect:
            print("Connecting to Rerun viewer…")
            rr.connect_grpc()
        else:
            rr.memory_recording()
        data = Transform3DInput.prepare(42, args.num_entities, args.num_time_steps)
        print("Logging…")
        log_transform3d_translation_mat3x3(data, static=args.static)
