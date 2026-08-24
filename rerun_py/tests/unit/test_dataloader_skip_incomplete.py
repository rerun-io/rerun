"""Tests for the iterable dataloader's missing-sample handling."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import torch
from rerun.experimental.dataloader._iterable_dataset import _raise_if_incomplete, _skip_incomplete
from rerun.experimental.dataloader._sample_index import SegmentMetadata
from rerun.experimental.dataloader._utils import Target

if TYPE_CHECKING:
    from collections.abc import Generator

    from rerun.experimental.dataloader.decoders._base import DecodedSample


def _stream(
    *samples: DecodedSample,
) -> Generator[DecodedSample, None, None]:
    yield from samples


def _complete(value: float = 1.0) -> DecodedSample:
    return {"image": torch.full((3, 2, 2), value), "action": torch.full((7,), value)}


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_drops_none_but_keeps_valid_empty_tensors() -> None:
    incomplete: DecodedSample = {"image": None, "action": torch.ones(7)}
    valid_empty: DecodedSample = {
        "image": torch.empty(0, 3, 2, 2),
        "action": torch.ones(7),
    }

    kept = list(_skip_incomplete(_stream(_complete(1.0), incomplete, _complete(2.0), valid_empty)))

    assert len(kept) == 3
    assert kept[-1] is valid_empty


def test_warns_once_per_missing_field() -> None:
    incomplete: DecodedSample = {"image": None, "action": torch.ones(7)}
    samples = _stream(*(incomplete for _ in range(5)))

    with pytest.warns(RuntimeWarning, match="field 'image' has no value") as record:
        assert list(_skip_incomplete(samples)) == []

    assert len(record) == 1


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_default_consecutive_skip_limit_is_1000() -> None:
    incomplete: DecodedSample = {"image": None, "action": torch.ones(7)}

    with pytest.raises(RuntimeError, match="Exceeded max_consecutive_skipped_samples=1000"):
        list(_skip_incomplete(_stream(*(incomplete for _ in range(1001)))))


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_allows_exactly_the_configured_number_of_consecutive_skipped_samples() -> None:
    incomplete: DecodedSample = {"image": None, "action": torch.ones(7)}

    kept = list(
        _skip_incomplete(
            _stream(incomplete, incomplete, _complete()),
            max_consecutive_skipped_samples=2,
        )
    )

    assert len(kept) == 1


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_valid_sample_resets_consecutive_skip_count() -> None:
    incomplete: DecodedSample = {"image": None, "action": torch.ones(7)}

    kept = list(
        _skip_incomplete(
            _stream(incomplete, _complete(1.0), incomplete, _complete(2.0)),
            max_consecutive_skipped_samples=1,
        )
    )

    assert len(kept) == 2


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_raises_after_consecutive_skip_limit_with_per_field_counts() -> None:
    missing_image: DecodedSample = {"image": None, "action": torch.ones(7)}
    missing_both: DecodedSample = {"image": None, "action": None}

    with pytest.raises(RuntimeError, match="Exceeded max_consecutive_skipped_samples=1") as error:
        list(
            _skip_incomplete(
                _stream(missing_image, missing_both),
                max_consecutive_skipped_samples=1,
            )
        )

    assert "2 total; missing fields: action=1, image=2" in str(error.value)


def test_rejects_a_negative_skip_budget() -> None:
    with pytest.raises(ValueError, match="max_consecutive_skipped_samples must be non-negative"):
        list(_skip_incomplete(_stream(), max_consecutive_skipped_samples=-1))


def test_closes_the_source_when_the_consumer_stops_early() -> None:
    closed = False

    def source() -> Generator[DecodedSample, None, None]:
        nonlocal closed
        try:
            while True:
                yield _complete()
        finally:
            closed = True

    samples = _skip_incomplete(source())
    next(samples)
    samples.close()

    assert closed


def test_manifest_replay_raises_when_a_required_field_is_missing() -> None:
    target = Target(
        segment=SegmentMetadata(segment_id="segment-a", index_start=0, index_end=10, num_samples=11),
        index_value=7,
        fetch_requests={},
    )

    with pytest.raises(RuntimeError, match="Required fields decoded to nothing: image") as error:
        _raise_if_incomplete({"image": None, "action": torch.ones(7)}, target, {"image"})

    assert "Segment: segment-a at 7" in str(error.value)


def test_manifest_replay_allows_an_optional_field_to_be_missing() -> None:
    target = Target(
        segment=SegmentMetadata(segment_id="segment-a", index_start=0, index_end=10, num_samples=11),
        index_value=7,
        fetch_requests={},
    )

    _raise_if_incomplete({"image": None, "action": torch.ones(7)}, target, {"action"})
