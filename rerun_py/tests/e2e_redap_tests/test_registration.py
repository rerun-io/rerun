from __future__ import annotations

import os
import tempfile
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from collections.abc import Iterator

    from rerun.catalog import CatalogClient


@pytest.fixture(scope="function")
def temp_empty_file() -> Iterator[str]:
    fd, tmp_path = tempfile.mkstemp(suffix=".rrd")
    os.close(fd)
    yield f"file://{tmp_path}"
    os.unlink(tmp_path)


@pytest.fixture(scope="function")
def temp_empty_directory() -> Iterator[str]:
    tmp_dir = tempfile.mkdtemp()
    yield f"file://{tmp_dir}"
    os.rmdir(tmp_dir)


def test_registration_invalidargs(
    catalog_client: CatalogClient, _temp_empty_file: str, temp_empty_directory: str
) -> None:
    """Tests the url property on the catalog and dataset."""

    ds = catalog_client.create_dataset(
        name="test_registration_invalidargs",
    )

    try:
        with pytest.raises(ValueError, match="no data sources to register"):
            ds.register_batch([])
        with pytest.raises(ValueError, match="no data sources to register"):
            ds.register_prefix(temp_empty_directory)
        # TODO(andrea): https://rerunio.slack.com/archives/C05694LC2EQ/p1764951439698349
        # with pytest.raises(ValueError, match="expected prefix / directory but got an object"):
        #     ds.register_prefix(temp_empty_file)
    finally:
        ds.delete()
