"""
Configuration file for pytest.

`conftest.py` is where pytest looks for fixtures and other customization/extensions.
"""

from __future__ import annotations

import dataclasses
import logging
import pathlib
import platform
import re
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.catalog import CatalogClient, TableEntry
from rerun.server import Server
from syrupy.extensions.amber import AmberSnapshotExtension

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun.catalog import DatasetEntry
    from syrupy import SnapshotAssertion

# Marker expressions for test profiles. Each profile defines a `-m`-style expression
# that is AND-combined with user-supplied `-m` flag (if any). Local is the default profile
# if nothing's specified
PROFILES: dict[str, str] = {
    "local": "",
    "dpf-docker": "not local_only",
    "dpf-stack": "not local_only or cloud_only",
}


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
    parser.addoption(
        "--resource-prefix",
        action="store",
        default=None,
        help="URI prefix for test resources (e.g., 's3://bucket/path/' for remote resources). "
        "If not provided, local file:// URIs to the resources directory will be used.",
    )
    parser.addoption(
        "--profile",
        action="store",
        default="local",
        choices=PROFILES.keys(),
        help="Test profile controlling which marker categories are auto-skipped. "
        "Choices: 'local' (default), 'dpf-docker' (skip local_only), "
        "'dpf-stack' (skip local_only), 'all' (skip nothing).",
    )
    parser.addoption(
        "--cloud",
        action="store",
        default=None,
        help="Cloud provider the tests are running against (e.g., 'aws', 'azure'). "
        "Used to skip cloud-specific tests like aws_only.",
    )


def pytest_configure(config: pytest.Config) -> None:
    """Register custom pytest markers."""

    config.addinivalue_line(
        "markers",
        "local_only: mark test as requiring local resources (e.g., uses RecordingStream to generate .rrd files on-the-fly)",
    )

    config.addinivalue_line(
        "markers",
        "aws_only: mark test as requiring AWS (e.g., uses S3 buckets directly)",
    )


def pytest_collection_modifyitems(config: pytest.Config, items: list[pytest.Item]) -> None:
    """Auto-skip tests based on the active profile and remote resource prefix."""

    # Profile-based filtering: AND-combine the profile expression with any user-supplied `-m`.
    profile_name = config.getoption("--profile")
    profile_expr = PROFILES[profile_name]
    if profile_expr:
        user_expr = config.option.markexpr
        if user_expr:
            config.option.markexpr = f"({profile_expr}) and ({user_expr})"
        else:
            config.option.markexpr = profile_expr

    # Automatically skip local-only tests when resource prefix is remote (not local file://)
    resource_prefix = config.getoption("--resource-prefix")
    is_local = resource_prefix is None or resource_prefix.startswith("file://")

    if not is_local:
        skip_marker = pytest.mark.skip(reason="Local-only test skipped when using remote resource prefix")
        for item in items:
            if "local_only" in item.keywords:
                item.add_marker(skip_marker)

    # Skip aws_only tests when not running on AWS
    cloud = config.getoption("--cloud")
    if cloud != "aws":
        reason = f"AWS-only test skipped on {cloud}" if cloud else "AWS-only test skipped (no --cloud specified)"
        skip_aws = pytest.mark.skip(reason=reason)
        for item in items:
            if "aws_only" in item.keywords:
                item.add_marker(skip_aws)


DATASET_NAME = "dataset"

# Test resources are stored locally in the e2e_redap_tests/resources directory
RESOURCES_DIR = pathlib.Path(__file__).parent / "resources"
TABLE_FILEPATH = RESOURCES_DIR / "simple_datatypes"


@pytest.fixture(scope="session")
def resource_prefix(request: pytest.FixtureRequest) -> str:
    """
    Get the URI prefix for test resources.

    By default, returns file:// URI to the local resources directory.
    Can be overridden with --resource-prefix for remote resources (e.g., s3://).
    """
    prefix = request.config.getoption("--resource-prefix")

    if prefix is None:
        # Default to local resources directory
        prefix = RESOURCES_DIR.absolute().as_uri() + "/"
    elif not prefix.endswith("/"):
        # Ensure prefix ends with trailing slash
        prefix = prefix + "/"

    return str(prefix)


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
def readonly_table_uri(resource_prefix: str) -> str:
    """
    Returns the URI to the read-only test table (simple_datatypes).

    Uses the resource_prefix, so it can point to local or remote (e.g., S3) resources.
    Tests should NOT write to this table.
    """
    return resource_prefix + "simple_datatypes"


@pytest.fixture(scope="session")
def catalog_client(request: pytest.FixtureRequest) -> Generator[CatalogClient, None, None]:
    """
    Return a `CatalogClient` instance connected to a test server.

    This is the core fixture that spins up a test server and returns the corresponding client. All other fixtures and
    tests should directly or indirectly depend on this.

    By default, this fixture creates a local OSS server. If the `--redap-url` option is provided, it will connect to
    the specified external server instead.

    This fixture has session scope, meaning a single server/connection is shared across all tests for better
    performance. Test isolation is maintained via the `entry_factory` fixture which uses test-specific prefixes and
    automatic cleanup.
    """
    redap_url = request.config.getoption("--redap-url")
    redap_token = request.config.getoption("--redap-token")

    if redap_url:
        # Connect to an external redap server
        client = CatalogClient(url=redap_url, token=redap_token)
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
        self._created_entries: list[DatasetEntry | TableEntry] = []

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

        For qualified names like "catalog.schema.table" or "schema.table",the prefix is applied only to the last
        component (the table name).
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

    def create_table(self, name: str, schema: pa.Schema, url: str | None = None) -> TableEntry:
        """Create a table with automatic cleanup. Mirrors CatalogClient.create_table()."""
        prefixed_name = self.apply_prefix(name)
        entry = self._client.create_table(prefixed_name, schema, url)
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

    Creates entries with test-specific prefixes to avoid collisions in external servers and automatically deletes them
    after the test completes.
    """

    prefix = f"{request.node.name}_"
    factory = EntryFactory(catalog_client, prefix)
    yield factory
    factory.cleanup()


@pytest.fixture(scope="session")
def readonly_test_dataset(catalog_client: CatalogClient, resource_prefix: str) -> Generator[DatasetEntry, None, None]:
    """
    Register a read-only dataset shared across the entire test session.

    This fixture creates a single dataset that is shared by all tests for better performance, particularly when testing
    against remote servers where dataset registration is expensive. Tests should NOT write to this dataset (use
    `entry_factory` to create test-specific datasets if you need to write).

    The dataset is automatically cleaned up at the end of the test session.
    """
    import uuid

    # Generate a session-specific prefix to avoid collisions across test runs
    session_id = uuid.uuid4().hex
    dataset_name = f"session_{session_id}_{DATASET_NAME}"

    # Create the dataset directly (not using entry_factory since it's function-scoped)
    ds = catalog_client.create_dataset(dataset_name)
    handle = ds.register_prefix(resource_prefix + "dataset")

    try:
        handle.wait(timeout_secs=50)
    except Exception as exc:
        # Attempt a cleanup just in case
        ds.delete()
        raise exc

    yield ds

    # Cleanup at session end
    try:
        ds.delete()
    except Exception as e:
        logging.warning("Could not delete readonly dataset %s: %s", dataset_name, e)


@dataclasses.dataclass
class PrefilledCatalog:
    factory: EntryFactory
    prefilled_dataset: DatasetEntry

    @property
    def client(self) -> CatalogClient:
        """Convenience property to access the underlying CatalogClient."""
        return self.factory.client

    def prefilled_tables(self) -> list[TableEntry]:
        """Returns a list of table entries that are prefilled in the catalog."""
        return [entry for entry in self.factory._created_entries if isinstance(entry, TableEntry)]


# TODO(ab): this feels somewhat ad hoc and should probably be replaced by dedicated local fixtures
@pytest.fixture(scope="function")
def prefilled_catalog(entry_factory: EntryFactory, readonly_table_uri: str) -> Generator[PrefilledCatalog, None, None]:
    """Sets up a catalog to server prefilled with a test dataset and tables associated to various (SQL) catalogs and schemas."""

    dataset = entry_factory.create_dataset(DATASET_NAME)
    handle = dataset.register_prefix(readonly_table_uri.rsplit("/", 1)[0] + "/dataset")
    handle.wait(timeout_secs=50)

    # Register the read-only table with different catalog/schema qualifications
    for table_name in ["simple_datatypes", "second_schema.second_table", "alternate_catalog.third_schema.third_table"]:
        entry_factory.register_table(table_name, readonly_table_uri)

    resource = PrefilledCatalog(entry_factory, dataset)
    yield resource


class RedactedIdSnapshotExtension(AmberSnapshotExtension):
    """
    Custom syrupy extension that redacts 16-byte hexadecimal IDs in snapshot output.

    This is useful for snapshot testing data that contains dynamic IDs like entry IDs
    or UUIDs that would otherwise cause snapshot mismatches on every run.
    """

    # Pattern to match 16-byte (32 character) hexadecimal strings
    _ID_PATTERN = re.compile(r"[0-9a-fA-F]{32}")

    def serialize(self, data: object, **kwargs: object) -> str:
        """Serialize data and redact any 16-byte hex IDs."""
        serialized = super().serialize(data, **kwargs)
        return self._ID_PATTERN.sub("***", serialized)


@pytest.fixture
def snapshot_redact_id(snapshot: SnapshotAssertion) -> SnapshotAssertion:
    """
    Snapshot fixture that redacts 16-byte hexadecimal IDs.

    Use this instead of `snapshot` when testing data containing dynamic IDs
    like entry IDs or UUIDs that should not affect snapshot matching.

    Example:
        def test_something(snapshot_redact_id):
            data = ["__bp_187E07A0DE3193C61d3d2ebb5a60e22b", "test"]
            assert data == snapshot_redact_id
            # Snapshot will contain: ["__bp_***", "test"]

    """
    return snapshot.use_extension(RedactedIdSnapshotExtension)
