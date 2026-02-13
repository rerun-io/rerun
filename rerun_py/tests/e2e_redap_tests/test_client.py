from __future__ import annotations

import signal
import urllib.parse
from typing import TYPE_CHECKING

import pytest
from rerun.catalog import CatalogClient

if TYPE_CHECKING:
    import types

    from .conftest import PrefilledCatalog


def test_urls(prefilled_catalog: PrefilledCatalog) -> None:
    """Tests the url property on the catalog and dataset."""

    catalog = prefilled_catalog.prefilled_dataset.catalog
    url = urllib.parse.urlparse(catalog.url)
    assert url.scheme in ("rerun", "rerun+http", "rerun+https")

    table_name = prefilled_catalog.factory.apply_prefix("simple_datatypes")
    table = prefilled_catalog.client.get_table(name=table_name)
    url = urllib.parse.urlparse(table.storage_url)

    assert url.path.endswith("/simple_datatypes") or url.path.endswith("/simple_datatypes/")


# TODO(#12122): It'd be nice if the connection timeout was configurable, so we would not have to wait for 30 seconds for this test.
@pytest.mark.skip
def test_network_unreachable() -> None:
    """Tests that the client raises an error when the server is unreachable."""

    def timeout_handler(_signal_num: int, _frame: types.FrameType | None) -> None:
        raise TimeoutError("the operation did not time out on time")

    signal.signal(signal.SIGALRM, timeout_handler)
    signal.alarm(60)  # Our connection timeout is 30 seconds (ehttp default). Let's be generous to avoid flakiness

    try:
        with pytest.raises(ConnectionError, match=r"failed to connect to server"):  # Adjust exception type as needed
            # This works because 192.0.2.0 is a reserved address block for documentation and examples.
            # ISPs should not route traffic to this block and just drop SYN packets.
            CatalogClient(url="rerun+http://192.0.2.1")
    finally:
        signal.alarm(0)  # Cancel the alarm
