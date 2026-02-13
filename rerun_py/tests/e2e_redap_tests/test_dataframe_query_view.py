from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from e2e_redap_tests.conftest import EntryFactory

# TODO(ab): quite obviously, there needs to be many more tests here.


@pytest.mark.parametrize("index", [None, "does_not_exist"])
def test_dataframe_query_empty_dataset(index: str | None, entry_factory: EntryFactory) -> None:
    ds = entry_factory.create_dataset("empty_dataset")
    df = ds.reader(index=index)

    assert df.count() == 0
