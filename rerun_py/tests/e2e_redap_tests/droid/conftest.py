from __future__ import annotations

import logging
import uuid
from typing import TYPE_CHECKING, Any
from urllib.parse import urlparse

import pytest
from rerun.catalog import CatalogClient, DatasetEntry

logger = logging.getLogger(__name__)

if TYPE_CHECKING:
    from collections.abc import Iterator

    from rerun.catalog import CatalogClient, DatasetEntry


@pytest.fixture(scope="package")
def droid_dataset_name(request: pytest.FixtureRequest) -> str:
    """Fixture to provide access to the droid dataset."""
    name: str = request.config.getoption("--droid-dataset")
    return name


@pytest.fixture(scope="package")
def droid_preregister_dataset(request: pytest.FixtureRequest) -> bool:
    should_register: bool = request.config.getoption("--droid-preregister-dataset")
    return should_register


@pytest.fixture(scope="package")
def aws_regional_dataset_manifest_path(region: str) -> str:
    """URI of the regional dataset manifest describing shared test datasets."""
    KNOWN_MANIFESTS: dict[str, str] = {
        "us-west-2": "s3://rerun-redap-datasets-pdx/droid.2025.07.15/manifest.json",
    }
    if region in KNOWN_MANIFESTS:
        return KNOWN_MANIFESTS[region]
    else:
        pytest.fail(f"Unknown region '{region}' for manifest path lookup")


# TODO(RR-3106) - we need to support Azure and AWS here and everywhere
@pytest.fixture(scope="package")
def aws_dataset_manifest(aws_regional_dataset_manifest_path: str) -> dict[str, Any]:
    """Parsed dataset manifest describing shared test datasets."""
    import json

    import boto3

    parsed = urlparse(aws_regional_dataset_manifest_path)
    if parsed.scheme != "s3":
        raise ValueError(f"Invalid S3 URL: {aws_regional_dataset_manifest_path}")

    bucket = parsed.netloc
    key = parsed.path.lstrip("/")
    s3 = boto3.client("s3")

    response = s3.get_object(Bucket=bucket, Key=key)
    manifest: dict[str, Any] = json.loads(response["Body"].read().decode("utf-8"))
    return manifest


@pytest.fixture(scope="package")
def aws_segments_to_register(aws_dataset_manifest: dict[str, Any], droid_dataset_name: str, region: str) -> list[str]:
    """Extract files to register from manifest for a specific dataset and region."""
    manifest = aws_dataset_manifest
    # Extract variant from dataset name (e.g., 'droid:sample50' -> 'sample50')
    if ":" not in droid_dataset_name:
        pytest.fail(f"Dataset name '{droid_dataset_name}' must be in format 'prefix:variant'")

    _, variant_name = droid_dataset_name.split(":", 1)

    variant_config = manifest.get("variants", {}).get(variant_name)
    if not variant_config:
        pytest.fail(f"Variant '{variant_name}' not found in manifest")

    regional_urls = manifest.get("regional_urls", {})
    if not regional_urls:
        raise ValueError("No regional_urls found in manifest")

    if region not in regional_urls:
        pytest.fail(f"Region '{region}' not found in regional_urls")

    base_url = regional_urls[region].rstrip("/")

    if "segments" in variant_config:
        segments = variant_config["segments"]
    elif "partitions" in variant_config:
        segments = variant_config["partitions"]
    else:
        raise ValueError(f"No segments found for variant '{variant_name}'")

    seg_urls = [f"{base_url}/{segment}" for segment in segments]
    return seg_urls


@pytest.fixture(scope="package")
def dataset(
    request: pytest.FixtureRequest,
    droid_dataset_name: str,
    droid_preregister_dataset: bool,
    catalog_client: CatalogClient,
) -> Iterator[DatasetEntry]:
    """Fixture to provide a pre-registered droid dataset."""

    if not droid_preregister_dataset:
        yield catalog_client.get_dataset(droid_dataset_name)
        return

    # only request the fixture if we really need it
    aws_segments_to_register: list[str] = request.getfixturevalue("aws_segments_to_register")
    # Create a unique dataset name to avoid collisions
    droid_dataset_name = f"{uuid.uuid4().hex}-{droid_dataset_name}"
    dataset_handle = catalog_client.create_dataset(droid_dataset_name)
    try:
        logger.info(f"Registering {len(aws_segments_to_register)} files for dataset '{droid_dataset_name}'")
        task_ids = dataset_handle.register(aws_segments_to_register)
        result = task_ids.wait(timeout_secs=600)
        assert len(result.segment_ids) == len(aws_segments_to_register), (
            f"Expected {len(aws_segments_to_register)} registered segments, got {len(result.segment_ids)}"
        )

        logger.info(f"Successfully registered dataset '{droid_dataset_name}'")
        yield dataset_handle
    finally:
        dataset_handle.delete()
