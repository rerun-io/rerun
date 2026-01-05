from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


@pytest.mark.creates_table
def test_entries_without_hidden(entry_factory: EntryFactory, snapshot: SnapshotAssertion) -> None:
    """Test that entries(), datasets(), and tables() exclude hidden entries by default."""
    client = entry_factory.client

    # Capture entries before creating test entries
    datasets_before = {d.name for d in client.datasets()}
    tables_before = {t.name for t in client.tables()}
    entries_before = {e.name for e in client.entries()}

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table("test_table", pa.schema([pa.field("col", pa.int32())]))

    # Get entries after - should only show user-created entries (no hidden)
    datasets_after = {d.name for d in client.datasets()}
    tables_after = {t.name for t in client.tables()}
    entries_after = {e.name for e in client.entries()}

    # Diff to find newly created entries
    prefix = entry_factory.prefix
    new_datasets = sorted([d.removeprefix(prefix) for d in datasets_after - datasets_before])
    new_tables = sorted([t.removeprefix(prefix) for t in tables_after - tables_before])
    new_entries = sorted([e.removeprefix(prefix) for e in entries_after - entries_before])

    assert new_datasets == snapshot
    assert new_tables == snapshot
    assert new_entries == snapshot


@pytest.mark.creates_table
def test_entries_with_hidden(entry_factory: EntryFactory, snapshot_redact_id: SnapshotAssertion) -> None:
    """Test that entries(), datasets(), and tables() include hidden entries when include_hidden=True."""
    client = entry_factory.client

    # Capture entries before creating test entries (with hidden)
    datasets_before = {d.name for d in client.datasets(include_hidden=True)}
    tables_before = {t.name for t in client.tables(include_hidden=True) if not t.name.startswith("__entries")}
    entries_before = {e.name for e in client.entries(include_hidden=True) if not e.name.startswith("__entries")}

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table("test_table", pa.schema([pa.field("col", pa.int32())]))

    # Get entries after with hidden - should include blueprint datasets and system tables
    datasets_after = {d.name for d in client.datasets(include_hidden=True)}
    tables_after = {t.name for t in client.tables(include_hidden=True)}
    entries_after = {e.name for e in client.entries(include_hidden=True)}

    # Diff to find newly created entries (including hidden ones like blueprint datasets)
    prefix = entry_factory.prefix
    new_datasets = sorted([d.removeprefix(prefix) for d in datasets_after - datasets_before])
    new_tables = sorted([t.removeprefix(prefix) for t in tables_after - tables_before])
    new_entries = sorted([e.removeprefix(prefix) for e in entries_after - entries_before])

    assert new_datasets == snapshot_redact_id
    assert new_tables == snapshot_redact_id
    assert new_entries == snapshot_redact_id


@pytest.mark.creates_table
def test_entry_names_without_hidden(entry_factory: EntryFactory, snapshot: SnapshotAssertion) -> None:
    """Test that entry_names(), dataset_names(), and table_names() exclude hidden entries by default."""
    client = entry_factory.client

    # Capture names before creating test entries
    dataset_names_before = set(client.dataset_names())
    table_names_before = set(client.table_names())
    entry_names_before = set(client.entry_names())

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table("test_table", pa.schema([pa.field("col", pa.int32())]))

    # Get names after - should only show user-created entries (no hidden)
    dataset_names_after = set(client.dataset_names())
    table_names_after = set(client.table_names())
    entry_names_after = set(client.entry_names())

    # Diff to find newly created entries
    prefix = entry_factory.prefix
    new_dataset_names = sorted([d.removeprefix(prefix) for d in dataset_names_after - dataset_names_before])
    new_table_names = sorted([t.removeprefix(prefix) for t in table_names_after - table_names_before])
    new_entry_names = sorted([e.removeprefix(prefix) for e in entry_names_after - entry_names_before])

    assert new_dataset_names == snapshot
    assert new_table_names == snapshot
    assert new_entry_names == snapshot


@pytest.mark.creates_table
def test_entry_names_with_hidden(entry_factory: EntryFactory, snapshot_redact_id: SnapshotAssertion) -> None:
    """Test that entry_names(), dataset_names(), and table_names() include hidden entries when include_hidden=True."""
    client = entry_factory.client

    # Capture names before creating test entries (with hidden)
    dataset_names_before = set(client.dataset_names(include_hidden=True))
    table_names_before = {t for t in client.table_names(include_hidden=True) if not t.startswith("__entries")}
    entry_names_before = {e for e in client.entry_names(include_hidden=True) if not e.startswith("__entries")}

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table("test_table", pa.schema([pa.field("col", pa.int32())]))

    # Get names after with hidden - should include blueprint datasets and system tables
    dataset_names_after = set(client.dataset_names(include_hidden=True))
    table_names_after = set(client.table_names(include_hidden=True))
    entry_names_after = set(client.entry_names(include_hidden=True))

    # Diff to find newly created entries (including hidden ones like blueprint datasets)
    prefix = entry_factory.prefix
    new_dataset_names = sorted([d.removeprefix(prefix) for d in dataset_names_after - dataset_names_before])
    new_table_names = sorted([t.removeprefix(prefix) for t in table_names_after - table_names_before])
    new_entry_names = sorted([e.removeprefix(prefix) for e in entry_names_after - entry_names_before])

    assert new_dataset_names == snapshot_redact_id
    assert new_table_names == snapshot_redact_id
    assert new_entry_names == snapshot_redact_id


def test_entry_eq(entry_factory: EntryFactory) -> None:
    """Test that entries support `in` via it's `__eq__` implementation."""

    client = entry_factory.client

    ds1 = entry_factory.create_dataset("ds1")
    ds2 = entry_factory.create_dataset("ds2")

    entries = client.entries()

    assert ds1 in entries
    assert ds2 in entries
    assert ds1.id in entries
    assert ds2.id in entries
    assert ds1.name in entries
    assert ds2.name in entries

    assert "doesnt_exists" not in client.entries()
