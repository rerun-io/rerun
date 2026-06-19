from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry


def redact_segment_url(url: str, dataset: DatasetEntry) -> str:
    """Replace the dynamic origin and dataset_id in a segment URL with placeholders."""
    origin = dataset.catalog.url
    dataset_id = str(dataset.id)
    return url.replace(origin, "<ORIGIN>").replace(dataset_id, "<DATASET_ID>")
