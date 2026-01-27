from __future__ import annotations

import datafusion
import pyarrow as pa
from rerun.experimental._viewer_client import _to_record_batch


def test_to_record_batch_single_record_batch() -> None:
    """Single RecordBatch is passed through unchanged."""
    batch = pa.record_batch({"col": [1, 2, 3]})
    result = _to_record_batch(batch)
    assert result.equals(batch)


def test_to_record_batch_list_of_record_batches() -> None:
    """List of RecordBatches is concatenated into one."""
    batch1 = pa.record_batch({"col": [1, 2]})
    batch2 = pa.record_batch({"col": [3, 4]})
    result = _to_record_batch([batch1, batch2])
    expected = pa.record_batch({"col": [1, 2, 3, 4]})
    assert result.equals(expected)


def test_to_record_batch_datafusion_dataframe() -> None:
    """Datafusion DataFrame is converted to a single RecordBatch."""
    ctx = datafusion.SessionContext()
    df = ctx.from_pydict({"col": [1, 2, 3]})
    result = _to_record_batch(df)
    assert result.num_rows == 3
    assert result.column("col").to_pylist() == [1, 2, 3]
