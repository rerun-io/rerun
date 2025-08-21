from __future__ import annotations

import os
import pathlib
import subprocess
import time
from typing import TYPE_CHECKING

import psutil
import pyarrow as pa
import pytest
import rerun as rr
from datafusion import col, functions as f

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun_bindings import DatasetEntry

CATALOG_URL = "rerun+http://localhost:51234"
DATASET_NAME = "dataset"

DATASET_FILEPATH = pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "rrd" / "dataset"


def shutdown_process(process: subprocess.Popen[str]) -> None:
    main_pid = process.pid

    # Teardown: kill the specific process and any child processes
    try:
        if psutil.pid_exists(main_pid):
            main_process = psutil.Process(main_pid)

            # Get all child processes
            children = main_process.children(recursive=True)

            # Terminate children
            for child in children:
                try:
                    child.terminate()
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass

            if process.stdout:
                process.stdout.close()
            if process.stderr:
                process.stderr.close()
            if process.stdin:
                process.stdin.close()

            # Terminate main process
            process.terminate()

            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=30)
        else:
            pass

    except Exception as e:
        print(f"Error during cleanup: {e}")


@pytest.fixture(scope="module")
def server_instance() -> Generator[tuple[subprocess.Popen[str], DatasetEntry], None, None]:
    assert DATASET_FILEPATH.is_dir()

    env = os.environ.copy()
    if "RUST_LOG" not in env:
        # Server can be noisy by default
        env["RUST_LOG"] = "warning"

    cmd = ["python", "-m", "rerun", "server", "--dataset", str(DATASET_FILEPATH)]
    server_process = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    time.sleep(0.5)  # Wait for rerun server to start to remove a logged warning

    client = rr.catalog.CatalogClient(CATALOG_URL)
    dataset = client.get_dataset(name=DATASET_NAME)

    resource = (server_process, dataset)
    yield resource

    shutdown_process(server_process)


def test_df_aggregation(server_instance: tuple[subprocess.Popen[str], DatasetEntry]) -> None:
    (_process, dataset) = server_instance

    results = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .unnest_columns("/obj1:Points3D:positions")
        .aggregate(
            [],
            [
                f.min(col("/obj1:Points3D:positions")[0]).alias("min_x"),
                f.max(col("/obj1:Points3D:positions")[0]).alias("max_x"),
            ],
        )
        .collect()
    )

    assert results[0][0][0] == pa.scalar(1.0, type=pa.float32())
    assert results[0][1][0] == pa.scalar(50.0, type=pa.float32())


def test_partition_ordering(server_instance: tuple[subprocess.Popen[str], DatasetEntry]) -> None:
    (_process, dataset) = server_instance

    for time_index in ["time_1", "time_2", "time_3"]:
        streams = (
            dataset.dataframe_query_view(index=time_index, contents="/**")
            .fill_latest_at()
            .df()
            .select("rerun_partition_id", time_index)
            .execute_stream_partitioned()
        )

        prior_partition_ids = set()
        for rb_reader in streams:
            prior_partition = ""
            prior_timestamp = 0
            for rb in iter(rb_reader):
                rb = rb.to_pyarrow()
                for idx in range(rb.num_rows):
                    partition = rb[0][idx].as_py()
                    timestamp = rb[1][idx].as_py()

                    assert partition >= prior_partition
                    if partition == prior_partition and timestamp is not None:
                        assert timestamp >= prior_timestamp
                    else:
                        assert partition not in prior_partition_ids
                        prior_partition_ids.add(partition)

                    prior_partition = partition
                    if timestamp is not None:
                        prior_timestamp = timestamp
