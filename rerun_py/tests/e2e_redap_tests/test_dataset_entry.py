from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

from ._helpers import redact_segment_url

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry


def test_segment_url_with_datetime(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment URLs with Python datetime values."""

    segment_id = sorted(readonly_test_dataset.segment_ids())[0]
    url = readonly_test_dataset.segment_url(
        segment_id,
        "real_time",
        datetime.datetime(1970, 1, 1, 0, 0, 1, 234567, tzinfo=datetime.timezone.utc),
        datetime.datetime(1970, 1, 1, 0, 0, 2, 345678, tzinfo=datetime.timezone.utc),
    )

    assert redact_segment_url(url, readonly_test_dataset) == (
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc"
        "#when=real_time@1970-01-01T00:00:01.234567Z&time_selection="
        "real_time@1970-01-01T00:00:01.234567Z..1970-01-01T00:00:02.345678Z"
    )


def test_segment_url_with_timedelta(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment URLs with Python timedelta values."""

    segment_id = sorted(readonly_test_dataset.segment_ids())[0]
    url = readonly_test_dataset.segment_url(
        segment_id,
        "sim_time",
        datetime.timedelta(seconds=1, milliseconds=96),
        datetime.timedelta(seconds=2, milliseconds=97),
    )

    assert redact_segment_url(url, readonly_test_dataset) == (
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc"
        "#when=sim_time@+1.096s&time_selection=sim_time@+1.096s..+2.097s"
    )


def test_segment_url_with_sequence_start_only(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment URLs with only a sequence start value."""

    segment_id = sorted(readonly_test_dataset.segment_ids())[0]
    url = readonly_test_dataset.segment_url(segment_id, "step", 42)

    assert redact_segment_url(url, readonly_test_dataset) == (
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc#when=step@42"
    )


def test_segment_url_with_datetime_start_only(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment URLs with only a datetime start value."""

    segment_id = sorted(readonly_test_dataset.segment_ids())[0]
    url = readonly_test_dataset.segment_url(
        segment_id,
        "real_time",
        datetime.datetime(1970, 1, 1, 0, 0, 1, 234567, tzinfo=datetime.timezone.utc),
    )

    assert redact_segment_url(url, readonly_test_dataset) == (
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc"
        "#when=real_time@1970-01-01T00:00:01.234567Z"
    )


def test_segment_url_with_timedelta_start_only(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment URLs with only a timedelta start value."""

    segment_id = sorted(readonly_test_dataset.segment_ids())[0]
    url = readonly_test_dataset.segment_url(
        segment_id,
        "sim_time",
        datetime.timedelta(seconds=1, milliseconds=96),
    )

    assert redact_segment_url(url, readonly_test_dataset) == (
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc#when=sim_time@+1.096s"
    )
