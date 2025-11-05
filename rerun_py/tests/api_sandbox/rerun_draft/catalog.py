from __future__ import annotations

from rerun import catalog as _catalog


class CatalogClient:
    def __init__(self, address: str, token: str | None = None) -> None:
        self._inner = _catalog.CatalogClient(address, token)

    def __repr__(self) -> str:
        return repr(self._inner)

    def all_entries(self) -> list[Entry]:
        return self._inner.all_entries()

    def dataset_entries(self) -> list[DatasetEntry]:
        return self._inner.dataset_entries()

    def table_entries(self) -> list[TableEntry]:
        return self._inner.table_entries()

    def entry_names(self) -> list[str]:
        return self._inner.entry_names()

    def dataset_names(self) -> list[str]:
        return self._inner.dataset_names()

    def table_names(self) -> list[str]:
        return self._inner.table_names()

    def entries(self):
        return self._inner.entries()

    def datasets(self):
        return self._inner.datasets()

    def tables(self):
        return self._inner.tables()

    def get_dataset_entry(self, *, id: EntryId | str | None = None, name: str | None = None) -> DatasetEntry:
        return self._inner.get_dataset_entry(id=id, name=name)

    def get_table_entry(self, *, id: EntryId | str | None = None, name: str | None = None) -> TableEntry:
        return self._inner.get_table_entry(id=id, name=name)

    def get_dataset(self, *, id: EntryId | str | None = None, name: str | None = None) -> DatasetEntry:
        return self._inner.get_dataset(id=id, name=name)

    def get_table(self, *, id: EntryId | str | None = None, name: str | None = None):
        return self._inner.get_table(id=id, name=name)

    def create_dataset(self, name: str) -> DatasetEntry:
        return self._inner.create_dataset(name)

    def register_table(self, name: str, url: str) -> TableEntry:
        return self._inner.register_table(name, url)

    def create_table_entry(self, name: str, schema: pa.Schema, url: str) -> TableEntry:
        return self._inner.create_table_entry(name, schema, url)

    def write_table(self, name: str, batches, insert_mode: TableInsertMode) -> None:
        return self._inner.write_table(name, batches, insert_mode)

    def append_to_table(self, table_name: str, **named_params) -> None:
        return self._inner.append_to_table(table_name, **named_params)

    def do_global_maintenance(self) -> None:
        return self._inner.do_global_maintenance()

    @property
    def ctx(self):
        return self._inner.ctx


Entry = _catalog.Entry
DatasetEntry = _catalog.DatasetEntry
TableEntry = _catalog.TableEntry


AlreadyExistsError = _catalog.AlreadyExistsError
DataframeQueryView = _catalog.DataframeQueryView
EntryId = _catalog.EntryId
EntryKind = _catalog.EntryKind
NotFoundError = _catalog.NotFoundError
TableInsertMode = _catalog.TableInsertMode
Task = _catalog.Task
VectorDistanceMetric = _catalog.VectorDistanceMetric
