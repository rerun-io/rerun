from __future__ import annotations

from dataclasses import asdict
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from rerun.catalog import CatalogClient


@pytest.mark.local_only
def test_oss_storage_location_discovery_is_unimplemented(request: pytest.FixtureRequest) -> None:
    if request.config.getoption("--redap-url"):
        pytest.skip("requires the local OSS server")

    catalog_client: CatalogClient = request.getfixturevalue("catalog_client")
    with pytest.raises(RuntimeError, match="storage locations are not supported"):
        catalog_client._get_storage_locations()


def test_storage_location_discovery(request: pytest.FixtureRequest) -> None:
    if not request.config.getoption("--storage-location-fixture"):
        pytest.skip("requires Hub with the storage-keychain fixture mounted")

    catalog_client: CatalogClient = request.getfixturevalue("catalog_client")
    locations = catalog_client._get_storage_locations()

    assert asdict(locations) == {
        "locations": [
            {
                "alias": "archive",
                "display_name": "Read-only archive",
                "region": "eu-west-1",
                "bucket_name": "e2e-archive",
                "prefix": "recordings",
                "read_only": True,
            },
            {
                "alias": "uploads",
                "display_name": None,
                "region": "us-east-1",
                "bucket_name": "e2e-uploads",
                "prefix": None,
                "read_only": False,
            },
        ],
        "default_alias": "uploads",
    }
