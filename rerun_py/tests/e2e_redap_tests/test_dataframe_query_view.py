from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry

    from e2e_redap_tests.conftest import EntryFactory


# TODO(ab): quite obviously, there needs to be many more tests here.


def test_dataframe_query_static_empty_dataset(entry_factory: EntryFactory) -> None:
    ds = entry_factory.create_dataset("empty_dataset")
    df = ds.reader(index=None)

    assert df.count() == 0


def test_dataframe_query_unknown_index_errors(readonly_test_dataset: DatasetEntry) -> None:
    """Querying with an unknown index should raise an error."""

    with pytest.raises(Exception, match="does not exist in the dataset"):
        readonly_test_dataset.reader(index="does_not_exist")
