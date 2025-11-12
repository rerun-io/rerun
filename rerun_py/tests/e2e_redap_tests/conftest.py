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
from rerun.catalog import CatalogClient
from rerun.server import Server

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun.catalog import DatasetEntry


def pytest_addoption(parser: pytest.Parser) -> None:
    """Add custom command-line options for configuring the test server."""
    parser.addoption(
        "--redap-url",
        action="store",
        default=None,
        help="URL of an external redap server to connect to. If not provided, a local OSS server will be started.",
    )
    parser.addoption(
        "--redap-token",
        action="store",
        default=None,
        help="Authentication token for the redap server (optional).",
    )


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
def catalog_client(request: pytest.FixtureRequest) -> Generator[CatalogClient, None, None]:
    """
    Return a `CatalogClient` instance connected to a test server.

    This is the core fixture that spins up a test server and returns the corresponding client. All other fixtures and
    tests should directly or indirectly depend on this.

    By default, this fixture creates a local OSS server. If the `--redap-url` option is provided, it will connect to
    the specified external server instead.
    """
    redap_url = request.config.getoption("--redap-url")
    redap_token = request.config.getoption("--redap-token")

    if redap_url:
        # Connect to an external redap server
        client = CatalogClient(address=redap_url, token=redap_token)
        yield client
        # No cleanup needed for external server
    else:
        # Create a local OSS server
        server = Server()
        yield server.client()
        server.shutdown()


@pytest.fixture(scope="function")
def test_dataset(catalog_client: CatalogClient) -> Generator[DatasetEntry, None, None]:
    """
    Register a dataset and returns the corresponding `DatasetEntry`.

    Convenient for tests which focus on a single test dataset.
    """
    assert DATASET_FILEPATH.is_dir()

    ds = catalog_client.create_dataset(DATASET_NAME)
    ds.register_prefix(DATASET_FILEPATH.as_uri())

    yield ds


@dataclasses.dataclass
class PrefilledCatalog:
    client: CatalogClient
    dataset: DatasetEntry


# TODO(ab): this feels somewhat ad hoc and should probably be replaced by dedicated local fixtures
@pytest.fixture(scope="function")
def prefilled_catalog(
    catalog_client: CatalogClient, table_filepath: pathlib.Path
) -> Generator[PrefilledCatalog, None, None]:
    """Sets up a catalog to server prefilled with a test dataset and tables associated to various (SQL) catalogs and schemas."""

    assert DATASET_FILEPATH.is_dir()
    assert table_filepath.is_dir()

    dataset = catalog_client.create_dataset(DATASET_NAME)
    dataset.register_prefix(DATASET_FILEPATH.as_uri())

    for table_name in ["simple_datatypes", "second_schema.second_table", "alternate_catalog.third_schema.third_table"]:
        catalog_client.register_table(table_name, table_filepath.as_uri())

    resource = PrefilledCatalog(catalog_client, dataset)
    yield resource
