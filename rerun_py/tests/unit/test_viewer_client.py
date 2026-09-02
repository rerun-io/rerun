from __future__ import annotations

from datetime import datetime, timezone

import datafusion
import pyarrow as pa
import pytest
from rerun._arrow import to_record_batch
from rerun.experimental import ViewerClient

import rerun_bindings  # noqa: TID251


def _capturing_viewer(monkeypatch: pytest.MonkeyPatch) -> tuple[ViewerClient, list[tuple[str | None, int, bool]]]:
    calls: list[tuple[str | None, int, bool]] = []

    class CapturingViewerClientInternal:
        def __init__(self, _url: str) -> None:
            pass

        def set_time_cursor(self, timeline: str | None, time: int, play: bool) -> None:
            calls.append((timeline, time, play))

    monkeypatch.setattr(rerun_bindings, "ViewerClientInternal", CapturingViewerClientInternal)
    return ViewerClient.connect(), calls


def test_to_record_batch_single_record_batch() -> None:
    """Single RecordBatch is passed through unchanged."""
    batch = pa.record_batch({"col": [1, 2, 3]})
    result = to_record_batch(batch)
    assert result.equals(batch)


def test_to_record_batch_list_of_record_batches() -> None:
    """List of RecordBatches is concatenated into one."""
    batch1 = pa.record_batch({"col": [1, 2]})
    batch2 = pa.record_batch({"col": [3, 4]})
    result = to_record_batch([batch1, batch2])
    expected = pa.record_batch({"col": [1, 2, 3, 4]})
    assert result.equals(expected)


def test_to_record_batch_datafusion_dataframe() -> None:
    """Datafusion DataFrame is converted to a single RecordBatch."""
    ctx = datafusion.SessionContext()
    df = ctx.from_pydict({"col": [1, 2, 3]})
    result = to_record_batch(df)
    assert result.num_rows == 3
    assert result.column("col").to_pylist() == [1, 2, 3]


# TODO(andreas): Add a setter/getter round-trip test once ViewerClient has a direct time-cursor getter.
def test_set_time_uses_active_timeline_when_omitted(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, calls = _capturing_viewer(monkeypatch)
    viewer.set_time(sequence=42)

    assert calls == [(None, 42, False)]


def test_set_time_converts_temporal_values(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, calls = _capturing_viewer(monkeypatch)
    viewer.set_time("elapsed", duration=1.5, play=True)
    viewer.set_time("capture_time", timestamp=datetime(1970, 1, 1, tzinfo=timezone.utc))

    assert calls == [("elapsed", 1_500_000_000, True), ("capture_time", 0, False)]


def test_set_time_requires_exactly_one_time(monkeypatch: pytest.MonkeyPatch) -> None:
    viewer, _calls = _capturing_viewer(monkeypatch)

    with pytest.raises(ValueError, match="exactly one"):
        viewer.set_time()  # type: ignore[call-overload]

    with pytest.raises(ValueError, match="exactly one"):
        viewer.set_time(sequence=1, duration=1.0)  # type: ignore[call-overload]
