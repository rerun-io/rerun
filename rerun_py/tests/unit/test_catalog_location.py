from __future__ import annotations

from unittest.mock import MagicMock

import pytest
from rerun.catalog import DatasetEntry


def test_deprecated_selector_warns_once_through_wrapper() -> None:
    internal = MagicMock()
    internal.blueprint_dataset.return_value.register.return_value.wait.return_value = ["blueprint"]

    with pytest.warns(DeprecationWarning, match="object_store_config") as caught:
        DatasetEntry(internal).register_blueprint(  # ty: ignore[deprecated]
            "s3://bucket/blueprint.rbl", object_store_config="robot-storage"
        )

    assert len(caught) == 1
    assert caught[0].filename == __file__
