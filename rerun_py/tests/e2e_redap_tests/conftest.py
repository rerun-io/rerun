"""
Configuration file for pytest.

`conftest.py` is where pytest looks for fixtures and other customization/extensions.
"""

from __future__ import annotations

import dataclasses
import logging
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

    from rerun.catalog import DatasetEntry, Entry, TableEntry


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


class EntryFactory:
    """
    Factory for creating catalog entries with automatic cleanup.

    Mirrors the CatalogClient API for entry creation methods but adds automatic resource naming and cleanup.
    """

    def __init__(self, client: CatalogClient, prefix: str) -> None:
        self._client = client
        self._prefix = prefix
        self._created_entries: list[Entry] = []

    @property
    def client(self) -> CatalogClient:
        """Underlying `CatalogClient` instance."""
        return self._client

    @property
    def prefix(self) -> str:
        """Prefix used for entries created by this factory."""
        return self._prefix

    def apply_prefix(self, name: str) -> str:
        """
        Apply prefix to a name, handling qualified names (A.B.C format).

        For qualified names like "catalog.schema.table" or "schema.table",
        the prefix is applied only to the last component (the table name).
        """
        if not self._prefix:
            return name

        parts = name.split(".")
        parts[-1] = f"{self._prefix}{parts[-1]}"
        return ".".join(parts)

    def create_dataset(self, name: str) -> DatasetEntry:
        """Create a dataset with automatic cleanup. Mirrors CatalogClient.create_dataset()."""
        prefixed_name = self.apply_prefix(name)
        entry = self._client.create_dataset(prefixed_name)
        self._created_entries.append(entry)
        return entry

    def create_table_entry(self, name: str, schema: pa.Schema, url: str) -> TableEntry:
        """Create a table entry with automatic cleanup. Mirrors CatalogClient.create_table_entry()."""
        prefixed_name = self.apply_prefix(name)
        entry = self._client.create_table_entry(prefixed_name, schema, url)
        self._created_entries.append(entry)
        return entry

    def register_table(self, name: str, url: str) -> TableEntry:
        """Register a table with automatic cleanup. Mirrors CatalogClient.register_table()."""
        prefixed_name = self.apply_prefix(name)
        entry = self._client.register_table(prefixed_name, url)
        self._created_entries.append(entry)
        return entry

    def cleanup(self) -> None:
        """Delete all created entries in reverse order."""
        for entry in reversed(self._created_entries):
            entry_id = entry.id
            try:
                entry.delete()
            except Exception as e:
                logging.warning("Could not delete entry %s: %s", entry_id, e)


@pytest.fixture(scope="function")
def entry_factory(catalog_client: CatalogClient, request: pytest.FixtureRequest) -> Generator[EntryFactory, None, None]:
    """
    Factory for creating catalog entries with automatic cleanup.

    Creates entries with test-specific prefixes to avoid collisions in external servers,
    and automatically deletes them after the test completes.
    """

    prefix = f"{request.node.name}_"
    factory = EntryFactory(catalog_client, prefix)
    yield factory
    factory.cleanup()


@pytest.fixture(scope="function")
def test_dataset(entry_factory: EntryFactory) -> Generator[DatasetEntry, None, None]:
    """
    Register a dataset and returns the corresponding `DatasetEntry`.

    Convenient for tests which focus on a single test dataset.
    """
    assert DATASET_FILEPATH.is_dir()

    ds = entry_factory.create_dataset(DATASET_NAME)
    ds.register_prefix(DATASET_FILEPATH.as_uri())

    yield ds


@dataclasses.dataclass
class PrefilledCatalog:
    factory: EntryFactory
    dataset: DatasetEntry

    @property
    def client(self) -> CatalogClient:
        """Convenience property to access the underlying CatalogClient."""
        return self.factory.client


# TODO(ab): this feels somewhat ad hoc and should probably be replaced by dedicated local fixtures
@pytest.fixture(scope="function")
def prefilled_catalog(
    entry_factory: EntryFactory, table_filepath: pathlib.Path
) -> Generator[PrefilledCatalog, None, None]:
    """Sets up a catalog to server prefilled with a test dataset and tables associated to various (SQL) catalogs and schemas."""

    assert DATASET_FILEPATH.is_dir()
    assert table_filepath.is_dir()

    dataset = entry_factory.create_dataset(DATASET_NAME)
    dataset.register_prefix(DATASET_FILEPATH.as_uri())

    for table_name in ["simple_datatypes", "second_schema.second_table", "alternate_catalog.third_schema.third_table"]:
        entry_factory.register_table(table_name, table_filepath.as_uri())

    resource = PrefilledCatalog(entry_factory, dataset)
    yield resource
