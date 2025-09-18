from __future__ import annotations

import os
import pathlib
import platform
import subprocess
import time
from typing import TYPE_CHECKING

import psutil
import pyarrow as pa
import pytest
from datafusion import col, functions as f
from rerun.catalog import CatalogClient

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun_bindings import DatasetEntry

HOST = "localhost"
PORT = 51234
CATALOG_URL = f"rerun+http://{HOST}:{PORT}"
DATASET_NAME = "dataset"

DATASET_FILEPATH = pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "rrd" / "dataset"


@pytest.fixture(scope="session", autouse=True)
def setup_windows_tzdata() -> None:
    """
    Adds timezone data on Windows machines.

    Pyarrow requires timezone data to handle timestamps properly.
    Arrow can use the OS-provided timezone database on Mac and Linux
    but it requires this command to install tzdata for Windows.
    https://arrow.apache.org/docs/python/install.html#tzdata-on-windows
    """
    if platform.system() == "Windows":
        pa.util.download_tzdata_on_windows()


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


def wait_for_server_ready(timeout: int = 30) -> None:
    import socket

    def is_port_open() -> bool:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(1)
        try:
            result = sock.connect_ex((HOST, PORT))
            return result == 0
        finally:
            sock.close()

    # Wait for port to be open
    start_time = time.time()
    while time.time() - start_time < timeout:
        if is_port_open():
            break
        time.sleep(0.1)
    else:
        raise TimeoutError(f"Server port {PORT} not ready within {timeout}s")


@pytest.fixture(scope="module")
def server_instance() -> Generator[tuple[subprocess.Popen[str], CatalogClient, DatasetEntry], None, None]:
    assert DATASET_FILEPATH.is_dir()

    env = os.environ.copy()
    if "RUST_LOG" not in env:
        # Server can be noisy by default
        env["RUST_LOG"] = "warning"

    # TODO(#11173): pick a free port
    cmd = ["python", "-m", "rerun", "server", "--dataset", str(DATASET_FILEPATH)]
    server_process = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    try:
        wait_for_server_ready()
    except Exception as e:
        print(f"Error during waiting for server to start: {e}")

    client = CatalogClient(CATALOG_URL)
    dataset = client.get_dataset(name=DATASET_NAME)

    resource = (server_process, client, dataset)
    yield resource

    shutdown_process(server_process)


def test_df_count(server_instance: tuple[subprocess.Popen[str], CatalogClient, DatasetEntry]) -> None:
    """
    Tests count() on a dataframe which ensures we collect empty batches properly.

    See issue https://github.com/rerun-io/rerun/issues/10894 for additional context.
    """
    (_process, _client, dataset) = server_instance

    count = dataset.dataframe_query_view(index="time_1", contents="/**").df().count()

    assert count > 0


def test_df_aggregation(server_instance: tuple[subprocess.Popen[str], CatalogClient, DatasetEntry]) -> None:
    (_process, _client, dataset) = server_instance

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


def test_component_filtering(server_instance: tuple[subprocess.Popen[str], CatalogClient, DatasetEntry]) -> None:
    """
    Cover the case where a user specifies a component filter on the client.

    We also support push down filtering to take a `.filter()` on the dataframe gets
    pushed into the query. Verify these both give the same results and that we don't
    get any nulls in that column.
    """
    (_process, _client, dataset) = server_instance

    component_path = "/obj2:Points3D:positions"

    filter_on_query = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .filter_is_not_null(component_path)
        .df()
        .collect_partitioned()
    )

    filter_on_dataframe = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .filter(col(component_path).is_not_null())
        .collect_partitioned()
    )

    for outer in filter_on_dataframe:
        for inner in outer:
            column = inner.column(component_path)
            assert column.null_count == 0

    assert filter_on_query == filter_on_dataframe


def test_partition_ordering(server_instance: tuple[subprocess.Popen[str], CatalogClient, DatasetEntry]) -> None:
    (_process, _client, dataset) = server_instance

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

                    # Nanosecond timestamps cannot be converted using `as_py()`
                    timestamp = rb[1][idx]
                    timestamp = timestamp.value if hasattr(timestamp, "value") else timestamp.as_py()

                    assert partition >= prior_partition
                    if partition == prior_partition and timestamp is not None:
                        assert timestamp >= prior_timestamp
                    else:
                        assert partition not in prior_partition_ids
                        prior_partition_ids.add(partition)

                    prior_partition = partition
                    if timestamp is not None:
                        prior_timestamp = timestamp


def test_arrow_rb_reader(server_instance: tuple[subprocess.Popen[str], CatalogClient, DatasetEntry]) -> None:
    (_process, client, dataset) = server_instance

    time_index = "time_1"
    rb_reader = dataset.dataframe_query_view(index=time_index, contents="/**").to_arrow_reader()

    # Similar to the partition ordering test, data in the record batch reader
    # should be sorted by partition and time index *per record batch*.
    prior_partition_ids = set()
    for rb in iter(rb_reader):
        prior_partition = ""
        prior_timestamp = 0
        for idx in range(rb.num_rows):
            partition = rb[0][idx].as_py()

            # Nanosecond timestamps cannot be converted using `as_py()`
            timestamp = rb.column(time_index)[idx]
            timestamp = timestamp.value if hasattr(timestamp, "value") else timestamp.as_py()

            assert partition >= prior_partition
            if partition == prior_partition and timestamp is not None:
                assert timestamp >= prior_timestamp
            else:
                assert partition not in prior_partition_ids
                prior_partition_ids.add(partition)

            prior_partition = partition
            if timestamp is not None:
                prior_timestamp = timestamp

    for partition_batch in dataset.partition_table().to_arrow_reader():
        assert partition_batch.num_rows > 0

    # TODO(tsaucer) Once OSS server supports table entries, uncomment this test
    # for table_entry in client.table_entries()[0].to_arrow_reader():
    #     assert table_entry.num_rows > 0


def test_url_generation(server_instance: tuple[subprocess.Popen[str], CatalogClient, DatasetEntry]) -> None:
    from rerun.utilities.datafusion.functions import url_generation

    (_process, _client, dataset) = server_instance

    udf = url_generation.partition_url_with_timeref_udf(dataset, "time_1")

    results = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .with_column("url", udf(col("rerun_partition_id"), col("time_1")))
        .sort(col("rerun_partition_id"), col("time_1"))
        .limit(1)
        .select("url")
        .collect()
    )

    # Since the OSS server will generate a random dataset ID at startup, we can only check part of
    # the generated URL
    assert (
        "partition_id=0cd72aae349f46bc97540d144582ff15#when=time_1@2024-01-15T10:30:45.123457000Z"
        in results[0][0][0].as_py()
    )
