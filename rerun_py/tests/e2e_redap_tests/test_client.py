from __future__ import annotations

import signal
import urllib.parse
from typing import TYPE_CHECKING

import pytest
from rerun.catalog import CatalogClient

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


def test_network_unreachable() -> None:
    """Tests that the client raises an error when the server is unreachable."""

    def timeout_handler(_signal_num, _frame):
        raise TimeoutError("the operation did not time out on time")

    signal.signal(signal.SIGALRM, timeout_handler)
    # It'd be nice if the connection timeout was configurable, so we would not have to wait for 10 seconds for this test. Another day.
    signal.alarm(15)  # Our connection timeout is 10 seconds. Let's be generous to avoid flakiness

    try:
        with pytest.raises(ConnectionError, match=r"failed to connect to server"):  # Adjust exception type as needed
            # This works because 192.0.2.0 is a reserved address block for documentation and examples.
            # ISPs should not route traffic to this block and just drop SYN packets.
            CatalogClient(address="rerun+http://192.0.2.1")
    finally:
        signal.alarm(0)  # Cancel the alarm
