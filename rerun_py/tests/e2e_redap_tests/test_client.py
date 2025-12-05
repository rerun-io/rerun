from __future__ import annotations

import re
import signal
from typing import TYPE_CHECKING

import pytest
from rerun.catalog import CatalogClient

if TYPE_CHECKING:
    import types

    from .conftest import ServerInstance


def test_urls(server_instance: ServerInstance) -> None:
    """Tests the url property on the catalog and dataset."""

    catalog = server_instance.dataset.catalog
    assert re.match("^rerun\\+http://(localhost|127.0.0.1):[0-9]+$", catalog.url)

    table = server_instance.client.get_table_entry(name="simple_datatypes")
    assert re.match("^file:///[-_:./0-9a-zA-Z]+/simple_datatypes/$", table.storage_url)


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
            CatalogClient(address="rerun+http://192.0.2.1")
    finally:
        signal.alarm(0)  # Cancel the alarm
