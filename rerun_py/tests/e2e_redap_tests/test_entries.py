from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    from pathlib import Path

    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


@pytest.mark.creates_table
def test_entries_without_hidden(entry_factory: EntryFactory, tmp_path: Path, snapshot: SnapshotAssertion) -> None:
    """Test that entries(), datasets(), and tables() exclude hidden entries by default."""
    client = entry_factory.client

    # Capture entries before creating test entries
    datasets_before = {d.name for d in client.datasets()}
    tables_before = {t.name for t in client.tables()}
    entries_before = {e.name for e in client.entries()}

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table_entry("test_table", pa.schema([pa.field("col", pa.int32())]), tmp_path.as_uri())

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
def test_entries_with_hidden(
    entry_factory: EntryFactory, tmp_path: Path, snapshot_redact_id: SnapshotAssertion
) -> None:
    """Test that entries(), datasets(), and tables() include hidden entries when include_hidden=True."""
    client = entry_factory.client

    # Capture entries before creating test entries (with hidden)
    datasets_before = {d.name for d in client.datasets(include_hidden=True)}
    tables_before = {t.name for t in client.tables(include_hidden=True) if not t.name.startswith("__entries")}
    entries_before = {e.name for e in client.entries(include_hidden=True) if not e.name.startswith("__entries")}

    # Create test entries
    entry_factory.create_dataset("test_dataset")
    entry_factory.create_table_entry("test_table", pa.schema([pa.field("col", pa.int32())]), tmp_path.as_uri())

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
