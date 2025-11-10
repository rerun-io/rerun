from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from .conftest import ServerInstance


# TODO(ab): quite obviously, there needs to be many more tests here.


@pytest.mark.parametrize("index", [None, "does_not_exist"])
def test_dataframe_query_empty_dataset(index: str | None, server_instance: ServerInstance) -> None:
    client = server_instance.client

    ds = client.create_dataset("empty_dataset")

    df = ds.dataframe_query_view(index=index, contents="/**").df()

    assert df.count() == 0
