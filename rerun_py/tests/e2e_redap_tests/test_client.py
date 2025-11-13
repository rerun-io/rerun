from __future__ import annotations

import urllib.parse
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .conftest import PrefilledCatalog


def test_urls(prefilled_catalog: PrefilledCatalog) -> None:
    """Tests the url property on the catalog and dataset."""

    catalog = prefilled_catalog.dataset.catalog
    url = urllib.parse.urlparse(catalog.url)
    assert url.scheme in ("rerun", "rerun+http", "rerun+https")

    table_name = prefilled_catalog.factory.apply_prefix("simple_datatypes")
    table = prefilled_catalog.client.get_table_entry(name=table_name)
    url = urllib.parse.urlparse(table.storage_url)

    assert url.path.endswith("/simple_datatypes") or url.path.endswith("/simple_datatypes/")
