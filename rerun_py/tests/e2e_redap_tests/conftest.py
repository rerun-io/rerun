from __future__ import annotations

import os
import pathlib
import platform
import shutil
import socket
import subprocess
import tempfile
import time
from typing import TYPE_CHECKING

import psutil
import pyarrow as pa
import pytest
from rerun.catalog import CatalogClient

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun_bindings import DatasetEntry


HOST = "localhost"
DATASET_NAME = "dataset"

DATASET_FILEPATH = pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "rrd" / "dataset"
TABLE_FILEPATH = (
    pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "table" / "lance" / "simple_datatypes"
)


@pytest.fixture(scope="module")
def table_filepath() -> Generator[pathlib.Path, None, None]:
    """
    Copies test data to a temp directory.

    This is necessary because we have some unit tests that will modify the
    lance dataset. We do not wish this to pollute our repository.
    """
    # Create a temporary directory
    with tempfile.TemporaryDirectory() as temp_dir:
        temp_path = pathlib.Path(temp_dir)

        # Copy all test data to the temp directory
        shutil.copytree(TABLE_FILEPATH, temp_path / "simple_datatypes")

        # Yield the path to the copied data
        yield temp_path / "simple_datatypes"


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


def wait_for_server_ready(port: int, timeout: int = 30) -> None:
    def is_port_open() -> bool:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(1)
        try:
            result = sock.connect_ex((HOST, port))
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
        raise TimeoutError(f"Server port {port} not ready within {timeout}s")


class ServerInstance:
    def __init__(self, proc: subprocess.Popen[str], client: CatalogClient, dataset: DatasetEntry) -> None:
        self.proc = proc
        self.client = client
        self.dataset = dataset


@pytest.fixture(scope="function")
def server_instance(table_filepath: pathlib.Path) -> Generator[ServerInstance, None, None]:
    assert DATASET_FILEPATH.is_dir()
    assert table_filepath.is_dir()

    env = os.environ.copy()
    if "RUST_LOG" not in env:
        # Server can be noisy by default
        env["RUST_LOG"] = "warning"

    # Find a free port dynamically
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.bind((HOST, 0))
    port = sock.getsockname()[1]
    sock.close()

    catalog_url = f"rerun+http://{HOST}:{port}"

    cmd = [
        "python",
        "-m",
        "rerun",
        "server",
        "--dataset",
        str(DATASET_FILEPATH),
        "--table",
        str(table_filepath),
        "--table",
        f"second_schema.second_table={table_filepath}",
        "--table",
        f"alternate_catalog.third_schema.third_table={table_filepath}",
        f"--port={port}",
    ]
    server_process = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    try:
        wait_for_server_ready(port)
    except Exception as e:
        print(f"Error during waiting for server to start: {e}")

    client = CatalogClient(catalog_url)
    dataset = client.get_dataset(name=DATASET_NAME)

    resource = ServerInstance(server_process, client, dataset)
    yield resource

    shutdown_process(server_process)
