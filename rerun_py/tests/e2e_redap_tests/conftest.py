"""
Configuration file for pytest.

`conftest.py` is where pytest looks for fixtures and other customization/extensions.
"""

from __future__ import annotations

import pathlib
import platform
import shutil
import tempfile
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.server import Server

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun.catalog import CatalogClient
    from rerun_bindings import DatasetEntry


DATASET_NAME = "dataset"

DATASET_FILEPATH = pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "rrd" / "dataset"
TABLE_FILEPATH = (
    pathlib.Path(__file__).parent.parent.parent.parent / "tests" / "assets" / "table" / "lance" / "simple_datatypes"
)


@pytest.fixture(scope="function")
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


class ServerInstance:
    def __init__(self, server: Server, client: CatalogClient, dataset: DatasetEntry) -> None:
        self.server = server
        self.client = client
        self.dataset = dataset


@pytest.fixture(scope="function")
def server_instance(table_filepath: pathlib.Path) -> Generator[ServerInstance, None, None]:
    assert DATASET_FILEPATH.is_dir()
    assert table_filepath.is_dir()

    # Create server with datasets and tables
    server = Server(
        datasets={DATASET_NAME: DATASET_FILEPATH},
        tables={
            "simple_datatypes": table_filepath,
            "second_schema.second_table": table_filepath,
            "alternate_catalog.third_schema.third_table": table_filepath,
        },
    )

    # Get client and dataset from the server
    client = server.client()
    dataset = client.get_dataset(name=DATASET_NAME)

    resource = ServerInstance(server, client, dataset)
    yield resource

    # Shutdown the server
    server.shutdown()
