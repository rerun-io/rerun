from __future__ import annotations

from typing import TYPE_CHECKING, Any

from rerun_bindings import TableEntryInternal

from . import (
    Entry,
)

if TYPE_CHECKING:
    import datafusion
    import pyarrow as pa


class TableEntry(Entry[TableEntryInternal]):
    """
    A table entry in the catalog.

    Note: this object acts as a table provider for DataFusion.
    """

    def __datafusion_table_provider__(self) -> Any:
        """Returns a DataFusion table provider capsule."""

        return self._internal.__datafusion_table_provider__()

    def df(self) -> datafusion.DataFrame:
        """Registers the table with the DataFusion context and return a DataFrame."""

        return self._internal.df()

    def to_arrow_reader(self) -> pa.RecordBatchReader:
        """Convert this table to a [`pyarrow.RecordBatchReader`][]."""

        return self._internal.to_arrow_reader()

    @property
    def storage_url(self) -> str:
        """The table's storage URL."""

        return self._internal.storage_url
