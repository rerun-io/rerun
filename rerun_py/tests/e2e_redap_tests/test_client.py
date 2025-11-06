from __future__ import annotations

import re
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .conftest import ServerInstance


def test_urls(server_instance: ServerInstance) -> None:
    """Tests the url property on the catalog and dataset."""

    catalog = server_instance.dataset.catalog
    assert re.match("^rerun\\+http://(localhost|127.0.0.1):[0-9]+$", catalog.url)

    table = server_instance.client.get_table_entry(name="simple_datatypes")
    assert re.match("^file:///[-_./0-9a-zA-Z]+/simple_datatypes/$", table.storage_url)
