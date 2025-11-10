from __future__ import annotations

from typing import TYPE_CHECKING


if TYPE_CHECKING:
    from .conftest import ServerInstance


# TODO(ab): quite obviously, there needs to be many more tests here.


def test_dataframe_query_empty_dataset_static(server_instance: ServerInstance) -> None:
    client = server_instance.client

    ds = client.create_dataset("empty_dataset")

    df = ds.dataframe_query_view(index=None, contents="/**").df()

    assert df.count() == 0


def test_dataframe_query_empty_dataset(server_instance: ServerInstance) -> None:
    client = server_instance.client

    ds = client.create_dataset("empty_dataset")

    df = ds.dataframe_query_view(index="does_not_exist", contents="/**").df()

    assert df.count() == 0
