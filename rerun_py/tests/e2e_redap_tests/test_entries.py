from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


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


def test_entries_with_hidden(entry_factory: EntryFactory) -> None:
    """
    Test that entries(), datasets(), and tables() reveal more entries when include_hidden=True.

    The exact set of hidden entries (blueprint datasets, system tables, …) is an implementation detail,
    so we only assert that the hidden listing is a superset of the visible one — and strictly larger for
    datasets/entries, since creating a dataset also creates hidden blueprint datasets.
    """
    client = entry_factory.client

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table("test_table", pa.schema([pa.field("col", pa.int32())]))

    # Capture entries creating test entries, both visible-only and with hidden.
    datasets = {d.name for d in client.datasets()}
    datasets_hidden = {d.name for d in client.datasets(include_hidden=True)}
    tables = {t.name for t in client.tables()}
    tables_hidden = {t.name for t in client.tables(include_hidden=True)}
    entries = {e.name for e in client.entries()}
    entries_hidden = {e.name for e in client.entries(include_hidden=True)}

    # include_hidden reveals everything the visible listing does, plus hidden implementation-detail entries.
    assert datasets_hidden >= datasets
    assert tables_hidden >= tables
    assert entries_hidden >= entries


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


def test_entry_names_with_hidden(entry_factory: EntryFactory) -> None:
    """
    Test that entry_names(), dataset_names(), and table_names() reveal more entries when include_hidden=True.

    The exact set of hidden entries (blueprint datasets, system tables, …) is an implementation detail,
    so we only assert that the hidden listing is a superset of the visible one — and strictly larger for
    datasets/entries, since creating a dataset also creates hidden blueprint datasets.
    """
    client = entry_factory.client

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table("test_table", pa.schema([pa.field("col", pa.int32())]))

    # Capture names creating test entries, both visible-only and with hidden.
    dataset_names = set(client.dataset_names())
    dataset_names_hidden = set(client.dataset_names(include_hidden=True))
    table_names = set(client.table_names())
    table_names_hidden = set(client.table_names(include_hidden=True))
    entry_names = set(client.entry_names())
    entry_names_hidden = set(client.entry_names(include_hidden=True))

    # include_hidden reveals everything the visible listing does, plus hidden implementation-detail entries.
    assert dataset_names_hidden >= dataset_names
    assert table_names_hidden >= table_names
    assert entry_names_hidden >= entry_names


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
