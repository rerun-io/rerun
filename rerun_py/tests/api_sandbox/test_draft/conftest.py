from __future__ import annotations

import sys
from pathlib import Path
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    from collections.abc import Iterator

RERUN_DRAFT_PATH = str(Path(__file__).parent)

if RERUN_DRAFT_PATH not in sys.path:
    sys.path.insert(0, RERUN_DRAFT_PATH)

import rerun_draft as rr  # noqa: E402


@pytest.fixture(scope="function")
def populated_client(simple_dataset_prefix: Path) -> Iterator[rr.catalog.CatalogClient]:
    """Create a temporary dataset prefix with a few simple recordings."""

    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("basic_dataset")
        ds.register_prefix(simple_dataset_prefix.as_uri())

        # TODO(jleibs): Consider attaching this metadata table directly to the dataset
        # and automatically joining it by default
        meta = client.create_table(
            "basic_dataset_metadata",
            pa.schema([
                ("rerun_segment_id", pa.string()),
                ("success", pa.bool_()),
            ]),
        )

        meta.append(
            rerun_segment_id=["simple_recording_0", "simple_recording_2"],
            success=[True, False],
        )

        yield client


@pytest.fixture(scope="function")
def populated_client_complex(complex_dataset_prefix: Path) -> Iterator[rr.catalog.CatalogClient]:
    """Create a temporary dataset prefix with a few simple recordings."""

    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        # TODO(jleibs): Consider attaching this metadata table directly to the dataset
        # and automatically joining it by default
        meta = client.create_table(
            "complex_dataset_metadata",
            pa.schema([
                ("rerun_segment_id", pa.string()),
                ("success", pa.bool_()),
            ]),
        )

        meta.append(
            rerun_segment_id=["complex_recording_1", "complex_recording_2", "complex_recording_3"],
            success=[True, False, True],
        )

        yield client


@pytest.fixture(scope="function")
def basic_dataset(populated_client: rr.catalog.CatalogClient) -> Iterator[rr.catalog.DatasetEntry]:
    yield populated_client.get_dataset(name="basic_dataset")


@pytest.fixture(scope="function")
def basic_metadata(populated_client: rr.catalog.CatalogClient) -> Iterator[rr.catalog.TableEntry]:
    yield populated_client.get_table(name="basic_dataset_metadata")


@pytest.fixture(scope="function")
def complex_dataset(populated_client_complex: rr.catalog.CatalogClient) -> Iterator[rr.catalog.DatasetEntry]:
    yield populated_client_complex.get_dataset(name="complex_dataset")


@pytest.fixture(scope="function")
def complex_metadata(populated_client_complex: rr.catalog.CatalogClient) -> Iterator[rr.catalog.TableEntry]:
    yield populated_client_complex.get_table(name="complex_dataset_metadata")
