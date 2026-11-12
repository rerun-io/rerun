from __future__ import annotations

from unittest.mock import MagicMock

import pytest
from inline_snapshot import snapshot
from rerun.catalog import CatalogClient, DatasetEntry, StorageLocation, StorageLocations


def test_storage_location_descriptors() -> None:
    internal = MagicMock()
    internal.get_storage_locations.return_value = {
        "locations": [
            {
                "alias": "robot-storage-eu",
                "display_name": "Read-only storage",
                "region": "eu-west-1",
                "bucket_name": "eu-bucket",
                "prefix": "recordings/",
                "read_only": True,
            },
            {
                "alias": "robot-storage-us",
                "display_name": None,
                "region": "us-east-1",
                "bucket_name": "us-bucket",
                "prefix": None,
                "read_only": False,
            },
        ],
        "default_alias": "robot-storage-us",
    }
    client = CatalogClient._from_internal(internal)

    locations = client._get_storage_locations()
    assert locations == StorageLocations(
        locations=[
            StorageLocation("robot-storage-eu", "Read-only storage", "eu-west-1", "eu-bucket", "recordings/", True),
            StorageLocation("robot-storage-us", None, "us-east-1", "us-bucket", None, False),
        ],
        default_alias="robot-storage-us",
    )
    assert locations.locations[0].bucket_name == "eu-bucket"
    internal.get_storage_locations.assert_called_once_with()


def test_storage_locations_repr() -> None:
    locations = StorageLocations(
        locations=[
            StorageLocation("west", None, "us-west-1", "uploads", None, False),
            StorageLocation("east", "Archive", "us-east-1", "archive", "rrd/", True),
        ],
        default_alias="west",
    )
    # NOLINT_START
    assert repr(locations) == snapshot("""\
StorageLocations
  Alias           Name     Region     Bucket   Prefix  Access
  --------------  -------  ---------  -------  ------  ----------
  east            Archive  us-east-1  archive  rrd/    read-only
  west (default)           us-west-1  uploads          read-write\
""")
    # NOLINT_END
    assert locations.locations[0].alias == "west"
    assert repr(StorageLocations(locations=[], default_alias="west")) == snapshot("StorageLocations(empty)")


def test_deprecated_selector_warns_once_through_wrapper() -> None:
    internal = MagicMock()
    internal.blueprint_dataset.return_value.register_blueprint.return_value = ("blueprint", None)

    with pytest.warns(DeprecationWarning, match="object_store_config") as caught:
        DatasetEntry(internal).register_blueprint(  # ty: ignore[deprecated]
            "s3://bucket/blueprint.rbl", object_store_config="robot-storage-eu"
        )

    assert len(caught) == 1
    assert caught[0].filename == __file__
