"""
End-to-end behavior of the SDK catalog and its bundled `datafusion.SessionContext`.

These tests document how table entries on the Rerun server are exposed through SQL and the
DataFusion catalog API.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    from datafusion import SessionContext

    from .conftest import EntryFactory


SCHEMA = pa.schema([("id", pa.int64()), ("name", pa.utf8())])


def test_unqualified_name_resolves_in_default_catalog(entry_factory: EntryFactory) -> None:
    """
    A bare-named entry is queryable as an unqualified SQL identifier.

    The entry lives in the default catalog (`datafusion`) and the default schema (`public`);
    DataFusion fills those in automatically when the user writes `SELECT * FROM my_table`.
    """
    table_name = entry_factory.apply_prefix("flat_select")
    entry_factory.create_table("flat_select", SCHEMA)

    ctx: SessionContext = entry_factory.client.ctx
    result = ctx.sql(f'SELECT COUNT(*) AS n FROM "{table_name}"').to_arrow_table()
    assert result.column("n")[0].as_py() == 0


def test_dotted_name_resolves_through_virtual_hierarchy(entry_factory: EntryFactory) -> None:
    """
    An entry whose name contains dots is exposed as a multi-part SQL reference.

    The server stores `my_catalog.my_schema.qualified_table` as a single flat name; the SDK
    parses it client-side into (catalog, schema, table) so SQL like
    `SELECT * FROM my_catalog.my_schema.qualified_table` resolves to the same entry.
    """
    full_name = entry_factory.apply_prefix("my_catalog.my_schema.qualified_select")
    entry_factory.create_table("my_catalog.my_schema.qualified_select", SCHEMA)

    ctx: SessionContext = entry_factory.client.ctx
    quoted = ".".join(f'"{p}"' for p in full_name.split("."))
    result = ctx.sql(f"SELECT COUNT(*) AS n FROM {quoted}").to_arrow_table()
    assert result.column("n")[0].as_py() == 0


def test_catalog_schema_table_navigation_returns_provider(entry_factory: EntryFactory) -> None:
    """
    `ctx.catalog(c).schema(s).table(t)` returns a DataFusion `TableProvider` for the entry.

    This is the imperative counterpart to the SQL form above: any entry reachable as `c.s.t`
    in SQL is also reachable by walking the catalog API. The returned provider exposes the
    entry's schema (and would be used to scan it).
    """
    full_name = entry_factory.apply_prefix("nav_catalog.nav_schema.nav_table")
    catalog, schema, leaf = full_name.split(".")

    entry_factory.create_table("nav_catalog.nav_schema.nav_table", SCHEMA)

    ctx: SessionContext = entry_factory.client.ctx
    table_provider = ctx.catalog(catalog).schema(schema).table(leaf)
    assert table_provider.schema.remove_metadata() == SCHEMA


def test_runtime_created_catalog_is_reachable_without_reconnect(entry_factory: EntryFactory) -> None:
    """
    Catalogs introduced after the `CatalogClient` was constructed are reachable immediately.

    The SDK does not bake a fixed catalog list at construction time; creating an entry whose
    multi-part name introduces a brand-new catalog (`late_catalog.late_schema.…`) makes that
    catalog queryable in the same session without rebuilding the client.
    """
    full_name = entry_factory.apply_prefix("late_catalog.late_schema.late_table")
    entry_factory.create_table("late_catalog.late_schema.late_table", SCHEMA)

    ctx: SessionContext = entry_factory.client.ctx
    quoted = ".".join(f'"{p}"' for p in full_name.split("."))
    result = ctx.sql(f"SELECT COUNT(*) AS n FROM {quoted}").to_arrow_table()
    assert result.column("n")[0].as_py() == 0


def test_missing_table_surfaces_error(entry_factory: EntryFactory) -> None:
    """
    A SQL reference to a non-existent table errors out rather than hanging or returning empty.

    The exact error type and message are intentionally not asserted: they vary across DataFusion
    versions and may surface as either "catalog not found" or "table not found". The contract
    documented here is only that *some* error is raised.
    """
    ctx: SessionContext = entry_factory.client.ctx
    with pytest.raises(Exception):  # noqa: B017 — error type may vary across DataFusion versions
        ctx.sql(f'SELECT * FROM "{entry_factory.apply_prefix("definitely_not_a_real_table")}"').collect()


def test_lazy_catalog_lookups_do_not_appear_in_catalog_names(entry_factory: EntryFactory) -> None:
    """
    Looking up a bad catalog name about must not subsequently appear in `ctx.catalog_names()`.

    The SDK's catalog list lazily mints a placeholder for any name the planner asks about (so
    that DataFusion can keep walking down to `schema(...).table(...)`, where the real
    name-filtered server check happens). Listing operations like `SHOW CATALOGS` and
    `INFORMATION_SCHEMA.schemata` should reflect only catalogs the server actually knows about
    plus any catalogs the user has explicitly registered, never the lazy probe-cache.
    """
    ctx: SessionContext = entry_factory.client.ctx

    phantom = entry_factory.apply_prefix("phantom_catalog")

    # Probe the typo'd name so any lazy cache populates.
    _ = ctx.catalog(phantom)

    assert phantom not in ctx.catalog_names(), (
        f"{phantom!r} leaked into catalog_names() after a lazy probe; lazy lookups must not "
        f"surface as listable catalogs"
    )
