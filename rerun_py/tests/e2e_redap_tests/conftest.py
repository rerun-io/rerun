"""
Configuration file for pytest.

`conftest.py` is where pytest looks for fixtures and other customization/extensions.
"""

from __future__ import annotations

import dataclasses
import pathlib
import platform
import shutil
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.server import Server

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun.catalog import CatalogClient, DatasetEntry


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


@pytest.fixture(scope="session")
def table_filepath(tmp_path_factory: pytest.TempPathFactory) -> Generator[pathlib.Path, None, None]:
    """
    Copies test data to a temp directory.

    This is necessary because we have some unit tests that will modify the
    lance dataset. We do not wish this to pollute our repository.
    """

    temp_dir = tmp_path_factory.mktemp("table_filepath")
    shutil.copytree(TABLE_FILEPATH, temp_dir / "simple_datatypes")
    yield temp_dir / "simple_datatypes"


@pytest.fixture(scope="function")
def catalog_client() -> Generator[CatalogClient, None, None]:
    server = Server()
    yield server.client()
    server.shutdown()


@pytest.fixture(scope="function")
def test_dataset(catalog_client: CatalogClient) -> Generator[DatasetEntry, None, None]:
    assert DATASET_FILEPATH.is_dir()

    ds = catalog_client.create_dataset(DATASET_NAME)
    ds.register_prefix(DATASET_FILEPATH.as_uri())

    yield ds


@dataclasses.dataclass
class PrefilledCatalog:
    client: CatalogClient
    dataset: DatasetEntry


@pytest.fixture(scope="function")
def prefilled_catalog(
    catalog_client: CatalogClient, table_filepath: pathlib.Path
) -> Generator[PrefilledCatalog, None, None]:
    assert DATASET_FILEPATH.is_dir()
    assert table_filepath.is_dir()

    dataset = catalog_client.create_dataset(DATASET_NAME)
    dataset.register_prefix(DATASET_FILEPATH.as_uri())

    for table_name in ["simple_datatypes", "second_schema.second_table", "alternate_catalog.third_schema.third_table"]:
        catalog_client.register_table(table_name, table_filepath.as_uri())

    resource = PrefilledCatalog(catalog_client, dataset)
    yield resource
