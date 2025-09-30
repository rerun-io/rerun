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
from rerun_bindings import EntryKind

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun_bindings import DatasetEntry

HOST = "localhost"
PORT = 51234
CATALOG_URL = f"rerun+http://{HOST}:{PORT}"
DATASET_NAME = "dataset"

DATASET_FILEPATH = pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "rrd" / "dataset"
TABLE_FILEPATH = (
    pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "table" / "lance" / "simple_datatypes"
)


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


class ServerInstance:
    def __init__(self, proc: subprocess.Popen[str], client: CatalogClient, dataset: DatasetEntry) -> None:
        self.proc = proc
        self.client = client
        self.dataset = dataset


@pytest.fixture(scope="module")
def server_instance() -> Generator[ServerInstance, None, None]:
    assert DATASET_FILEPATH.is_dir()
    assert TABLE_FILEPATH.is_dir()

    env = os.environ.copy()
    if "RUST_LOG" not in env:
        # Server can be noisy by default
        env["RUST_LOG"] = "warning"

    # TODO(#11173): pick a free port
    cmd = ["python", "-m", "rerun", "server", "--dataset", str(DATASET_FILEPATH), "--table", str(TABLE_FILEPATH)]
    server_process = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    try:
        wait_for_server_ready()
    except Exception as e:
        print(f"Error during waiting for server to start: {e}")

    client = CatalogClient(CATALOG_URL)
    dataset = client.get_dataset(name=DATASET_NAME)

    resource = ServerInstance(server_process, client, dataset)
    yield resource

    shutdown_process(server_process)


def test_df_count(server_instance: ServerInstance) -> None:
    """
    Tests count() on a dataframe which ensures we collect empty batches properly.

    See issue https://github.com/rerun-io/rerun/issues/10894 for additional context.
    """
    dataset = server_instance.dataset

    count = dataset.dataframe_query_view(index="time_1", contents="/**").df().count()

    assert count > 0


def test_df_aggregation(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

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


def test_component_filtering(server_instance: ServerInstance) -> None:
    """
    Cover the case where a user specifies a component filter on the client.

    We also support push down filtering to take a `.filter()` on the dataframe gets
    pushed into the query. Verify these both give the same results and that we don't
    get any nulls in that column.
    """
    dataset = server_instance.dataset

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


def test_partition_ordering(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

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


def test_tables_to_arrow_reader(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

    for rb in dataset.dataframe_query_view(index="time_1", contents="/**").to_arrow_reader():
        assert rb.num_rows > 0

    for partition_batch in dataset.partition_table().to_arrow_reader():
        assert partition_batch.num_rows > 0

    for table_entry in server_instance.client.table_entries()[0].to_arrow_reader():
        assert table_entry.num_rows > 0


def test_url_generation(server_instance: ServerInstance) -> None:
    from rerun.utilities.datafusion.functions import url_generation

    dataset = server_instance.dataset

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


def test_query_view_from_schema(server_instance: ServerInstance) -> None:
    """Verify Our Schema is sufficiently descriptive to extract all contents from dataset."""
    from rerun_bindings import IndexColumnDescriptor

    dataset = server_instance.dataset

    # TODO(nick): This only works for a single shared index column
    # We should consider if our schema is sufficiently descriptive for
    # multi-indices
    index_column = None
    for entry in dataset.schema():
        if isinstance(entry, IndexColumnDescriptor):
            index_column = entry.name
        else:
            local_index_column = index_column
            if entry.is_static:
                local_index_column = None
            contents = dataset.dataframe_query_view(
                index=local_index_column, contents={entry.entity_path: entry.component}
            ).df()
            assert contents.count() > 0


def test_query_lance_table(server_instance: ServerInstance) -> None:
    expected_table_name = "simple_datatypes"
    entries_table_name = "__entries"

    client = server_instance.client
    assert expected_table_name in client.table_names()
    assert entries_table_name in client.table_names()

    entries = client.table_entries()
    assert len(entries) == 2

    # No guarantee on order here
    if entries[0].name == expected_table_name:
        assert entries[1].name == entries_table_name
    else:
        assert entries[0].name == entries_table_name
        assert entries[1].name == expected_table_name

    assert entries[0].kind == EntryKind.TABLE
    assert entries[1].kind == EntryKind.TABLE

    tables = client.tables()
    assert tables.collect()[0].num_rows == 1

    table = client.get_table(name=expected_table_name)
    assert table.collect()[0].num_rows > 0

    entry = client.get_table_entry(name=expected_table_name)
    assert entry.name == expected_table_name
    assert entry.kind == EntryKind.TABLE


def test_dataset_schema_comparison_self_consistent(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

    schema_0 = dataset.schema()
    schema_1 = dataset.schema()
    set_diff = set(schema_0).symmetric_difference(schema_1)

    assert len(set_diff) == 0, f"Schema iterator is not self-consistent: {set_diff}"
    assert schema_0 == schema_1, "Schema is not self-consistent"

def test_datafusion_catalog_get_tables(server_instance: ServerInstance) -> None:
    ctx = server_instance.client.ctx

    # Verify we have the catalog provider and schema provider
    catalog_provider = ctx.catalog("datafusion")
    assert catalog_provider is not None

    schema_provider = catalog_provider.schema("public")
    assert schema_provider is not None

    # Note: as of DataFusion 50.0.0 this is not a DataFrame
    # but rather a python object that describes the table.
    table = schema_provider.table("simple_datatypes")
    assert table is not None

    # Get by table name since it should be in the default catalog/schema
    df = ctx.table("simple_datatypes")
    rb = df.collect()[0]
    assert rb.num_rows > 0

    # Get table by fully qualified name
    df = ctx.table("datafusion.public.simple_datatypes")
    rb = df.collect()[0]
    assert rb.num_rows > 0

    # Verify SQL parsing for catalog provider works as expected
    df = ctx.sql("SELECT * FROM simple_datatypes")
    rb = df.collect()[0]
    assert rb.num_rows > 0

    df = ctx.sql("SELECT * FROM datafusion.public.simple_datatypes")
    rb = df.collect()[0]
    assert rb.num_rows > 0
