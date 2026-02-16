from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
from datafusion import SessionContext, col, lit
from inline_snapshot import snapshot as inline_snapshot
from rerun.utilities.datafusion.functions.url_generation import segment_url

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry


def redact_segment_url(url: str, dataset: DatasetEntry) -> str:
    """Replace the dynamic origin and dataset_id in a segment URL with placeholders."""
    origin = dataset.catalog.url
    dataset_id = str(dataset.id)
    return url.replace(origin, "<ORIGIN>").replace(dataset_id, "<DATASET_ID>")


def collect_urls(result: list[pa.RecordBatch], dataset: DatasetEntry) -> list[str]:
    """Extract and redact all URL values from query results."""
    urls = []
    for batch in result:
        for url in batch.column("url"):
            urls.append(redact_segment_url(url.as_py(), dataset))
    return urls


def test_segment_url_simple(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment_url UDF without timestamp -- just segment_id to URL."""

    segment_ids = sorted(readonly_test_dataset.segment_ids())[:5]
    view = readonly_test_dataset.filter_segments(segment_ids)

    result = (
        view.segment_table()
        .with_column("url", segment_url(readonly_test_dataset))
        .sort(col("rerun_segment_id"))
        .select("url")
        .collect()
    )

    assert collect_urls(result, readonly_test_dataset) == inline_snapshot([
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=24598969c97a4154a1ad0a262ee31b97",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=3ee345b2e801448cace33a1097b9b49b",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=45e562f3abc24cfbbcf49ad30fa04b47",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=526f111faae1465d865d80e9a5c9eb6d",
    ])


def test_segment_url_with_timestamp(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment_url UDF with a Timestamp(ns) column joined via join_meta."""

    segment_ids = sorted(readonly_test_dataset.segment_ids())[:6]

    ctx = SessionContext()
    meta_batch = pa.RecordBatch.from_pydict({
        "rerun_segment_id": segment_ids,
        "my_timestamp": pa.array(
            [
                1_705_312_245_123_456_789,  # 2024-01-15T10:30:45.123456789Z
                1_705_312_365_000_000_000,  # 2024-01-15T10:32:45Z
                1_705_312_485_000_000_000,  # 2024-01-15T10:34:45Z
                1_705_312_605_000_000_000,  # 2024-01-15T10:36:45Z
                1_705_312_725_000_000_000,  # 2024-01-15T10:38:45Z
                None,
            ],
            type=pa.timestamp("ns"),
        ),
    })
    meta_df = ctx.from_arrow(meta_batch)

    view = readonly_test_dataset.filter_segments(segment_ids)
    segment_table = view.segment_table(join_meta=meta_df)

    result = (
        segment_table.with_column(
            "url",
            segment_url(
                readonly_test_dataset,
                timestamp_col="my_timestamp",
                timeline_name="my_timeline",
            ),
        )
        .sort(col("rerun_segment_id"))
        .select("url")
        .collect()
    )

    assert collect_urls(result, readonly_test_dataset) == inline_snapshot([
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc#when=my_timeline@2024-01-15T09:50:45.123456789Z",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=24598969c97a4154a1ad0a262ee31b97#when=my_timeline@2024-01-15T09:52:45Z",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=3ee345b2e801448cace33a1097b9b49b#when=my_timeline@2024-01-15T09:54:45Z",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=45e562f3abc24cfbbcf49ad30fa04b47#when=my_timeline@2024-01-15T09:56:45Z",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=526f111faae1465d865d80e9a5c9eb6d#when=my_timeline@2024-01-15T09:58:45Z",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=68224eead5ed40838b3f3bdb0edfd2b2",
    ])


def test_segment_url_with_literal_segment_id(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment_url UDF with a literal segment_id and multiple timestamps."""

    segment_id = sorted(readonly_test_dataset.segment_ids())[0]

    ctx = SessionContext()
    ts_batch = pa.RecordBatch.from_pydict({
        "my_timestamp": pa.array(
            [
                1_705_312_245_123_456_789,  # 2024-01-15T10:30:45.123456789Z
                1_705_312_365_000_000_000,  # 2024-01-15T10:32:45Z
                1_705_312_485_000_000_000,  # 2024-01-15T10:34:45Z
            ],
            type=pa.timestamp("ns"),
        ),
    })
    ts_df = ctx.from_arrow(ts_batch)

    result = (
        ts_df.with_column(
            "url",
            segment_url(
                readonly_test_dataset,
                segment_id_col=lit(segment_id),
                timestamp_col="my_timestamp",
                timeline_name="my_timeline",
            ),
        )
        .select("url")
        .collect()
    )

    assert collect_urls(result, readonly_test_dataset) == inline_snapshot([
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc#when=my_timeline@2024-01-15T09:50:45.123456789Z",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc#when=my_timeline@2024-01-15T09:52:45Z",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc#when=my_timeline@2024-01-15T09:54:45Z",
    ])


def test_segment_url_with_sequence(readonly_test_dataset: DatasetEntry) -> None:
    """Test segment_url UDF with an Int64 (sequence) timestamp column."""

    segment_ids = sorted(readonly_test_dataset.segment_ids())[:6]

    ctx = SessionContext()
    meta_batch = pa.RecordBatch.from_pydict({
        "rerun_segment_id": segment_ids,
        "my_seq": pa.array([10, 20, 30, 40, 50, None], type=pa.int64()),
    })
    meta_df = ctx.from_arrow(meta_batch)

    view = readonly_test_dataset.filter_segments(segment_ids)
    segment_table = view.segment_table(join_meta=meta_df)

    result = (
        segment_table.with_column(
            "url",
            segment_url(
                readonly_test_dataset,
                timestamp_col="my_seq",
                timeline_name="my_seq",
            ),
        )
        .sort(col("rerun_segment_id"))
        .select("url")
        .collect()
    )

    assert collect_urls(result, readonly_test_dataset) == inline_snapshot([
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=141a866deb2d49f69eb3215e8a404ffc#when=my_seq@10",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=24598969c97a4154a1ad0a262ee31b97#when=my_seq@20",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=3ee345b2e801448cace33a1097b9b49b#when=my_seq@30",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=45e562f3abc24cfbbcf49ad30fa04b47#when=my_seq@40",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=526f111faae1465d865d80e9a5c9eb6d#when=my_seq@50",
        "<ORIGIN>/dataset/<DATASET_ID>?segment_id=68224eead5ed40838b3f3bdb0edfd2b2",
    ])
