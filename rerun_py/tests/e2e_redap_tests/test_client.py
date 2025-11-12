from __future__ import annotations

import re
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .conftest import PrefilledCatalog


def test_urls(prefilled_catalog: PrefilledCatalog) -> None:
    """Tests the url property on the catalog and dataset."""

    catalog = prefilled_catalog.dataset.catalog
    assert re.match("^rerun\\+http://(localhost|127.0.0.1):[0-9]+$", catalog.url)

    table_name = prefilled_catalog.factory.apply_prefix("simple_datatypes")
    table = prefilled_catalog.client.get_table_entry(name=table_name)
    assert re.match("^file:///[-_:./0-9a-zA-Z]+/simple_datatypes/$", table.storage_url)
