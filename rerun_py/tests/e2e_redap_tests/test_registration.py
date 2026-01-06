from __future__ import annotations

import os
import tempfile
from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.catalog import SegmentRegistrationResult

if TYPE_CHECKING:
    from collections.abc import Callable, Iterator, Sequence
    from pathlib import Path

    from rerun.catalog import CatalogClient

    from e2e_redap_tests.conftest import EntryFactory


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


@pytest.fixture(scope="function")
def recording_factory(tmp_path: Path) -> Callable[[Sequence[str]], list[str]]:
    """
    Factory fixture for creating test recordings with known recording IDs.

    Returns a callable that takes a sequence of recording IDs and returns the
    corresponding file URIs.
    """

    def create_recordings(recording_ids: Sequence[str]) -> list[str]:
        uris = []
        for i, recording_id in enumerate(recording_ids):
            rrd_path = tmp_path / f"recording_{i}.rrd"
            with rr.RecordingStream(f"test_recording_{i}", recording_id=recording_id) as rec:
                rec.save(rrd_path)
                rec.log("points", rr.Points2D([[i, i]]))
                rec.flush()
            uris.append(rrd_path.absolute().as_uri())
        return uris

    return create_recordings


def test_registration_invalidargs(
    catalog_client: CatalogClient, temp_empty_file: str, temp_empty_directory: str
) -> None:
    """Tests the url property on the catalog and dataset."""

    ds = catalog_client.create_dataset(
        name="test_registration_invalidargs",
    )

    try:
        with pytest.raises(ValueError, match="no data sources to register"):
            ds.register([])
        with pytest.raises(ValueError, match="no rrd files found in"):
            ds.register_prefix(temp_empty_directory)
        with pytest.raises(ValueError, match="expected prefix / directory but got an object"):
            ds.register_prefix(temp_empty_file)
    finally:
        ds.delete()


@pytest.mark.local_only
def test_register_single_with_wait(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test registering a single recording using wait()."""
    recording_id = "01234567-0123-0123-0123-0123456789ab"
    uris = recording_factory([recording_id])

    ds = entry_factory.create_dataset("test_register_single")

    handle = ds.register(uris[0])
    result = handle.wait()

    assert len(result.segment_ids) == 1
    assert result.segment_ids[0] == recording_id


@pytest.mark.local_only
def test_register_single_with_iter_results(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test registering a single recording using iter_results()."""
    recording_id = "11111111-1111-1111-1111-111111111111"
    uris = recording_factory([recording_id])

    ds = entry_factory.create_dataset("test_register_iter")

    handle = ds.register(uris[0])
    results = list(handle.iter_results())

    assert len(results) == 1
    result = results[0]

    assert isinstance(result, SegmentRegistrationResult)
    assert result.uri == uris[0]
    assert result.segment_id == recording_id
    assert result.error is None
    assert result.is_success is True


@pytest.mark.local_only
def test_register_batch(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test registering multiple recordings in a single call."""
    recording_ids = [
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "cccccccc-cccc-cccc-cccc-cccccccccccc",
    ]
    uris = recording_factory(recording_ids)

    ds = entry_factory.create_dataset("test_register_batch")

    handle = ds.register(uris)
    result = handle.wait()

    assert len(result.segment_ids) == 3
    assert sorted(result.segment_ids) == sorted(recording_ids)


@pytest.mark.local_only
def test_register_batch_with_iter_results(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test batch registration with iter_results() streaming."""
    recording_ids = [
        "dddddddd-dddd-dddd-dddd-dddddddddddd",
        "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
        "ffffffff-ffff-ffff-ffff-ffffffffffff",
    ]
    uris = recording_factory(recording_ids)

    ds = entry_factory.create_dataset("test_batch_iter")

    handle = ds.register(uris)
    results = list(handle.iter_results())

    assert len(results) == 3

    for result in results:
        assert isinstance(result, SegmentRegistrationResult)
        assert result.is_success is True
        assert result.segment_id is not None
        assert result.error is None

    # Build expected mapping of uri -> segment_id
    expected_segment_ids = dict(zip(uris, recording_ids, strict=False))

    for result in results:
        assert result.segment_id == expected_segment_ids[result.uri]


@pytest.mark.local_only
def test_register_with_layer_name(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test registration with custom layer_name parameter."""
    recording_id = "22222222-2222-2222-2222-222222222222"
    uris = recording_factory([recording_id])

    ds = entry_factory.create_dataset("test_layer_name")

    handle = ds.register(uris[0], layer_name="custom_layer")
    result = handle.wait()

    assert len(result.segment_ids) == 1
    assert result.segment_ids[0] == recording_id


@pytest.mark.local_only
def test_register_batch_with_different_layers(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test batch registration with different layer names for each URI."""
    recording_ids = [
        "33333333-3333-3333-3333-333333333333",
        "44444444-4444-4444-4444-444444444444",
    ]
    uris = recording_factory(recording_ids)

    ds = entry_factory.create_dataset("test_diff_layers")

    handle = ds.register(uris, layer_name=["layer_a", "layer_b"])
    result = handle.wait()

    assert len(result.segment_ids) == 2
    assert sorted(result.segment_ids) == sorted(recording_ids)


@pytest.mark.local_only
def test_register_layer_name_length_mismatch(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test that mismatched layer_name list length raises ValueError."""
    recording_ids = [
        "55555555-5555-5555-5555-555555555555",
        "66666666-6666-6666-6666-666666666666",
        "77777777-7777-7777-7777-777777777777",
    ]
    uris = recording_factory(recording_ids)

    ds = entry_factory.create_dataset("test_mismatch")

    with pytest.raises(ValueError, match="must be the same length"):
        ds.register(uris, layer_name=["layer_a", "layer_b"])  # 3 URIs, 2 layers


# TODO(RR-3177): we should fix our server implementations such that this test passes
@pytest.mark.skip
@pytest.mark.local_only
def test_register_same_segment_id(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test that mismatched layer_name list length raises ValueError."""
    recording_ids = [
        "55555555-5555-5555-5555-555555555555",
        "66666666-6666-6666-6666-666666666666",
        "66666666-6666-6666-6666-666666666666",
        "77777777-7777-7777-7777-777777777777",
    ]
    uris = recording_factory(recording_ids)

    ds = entry_factory.create_dataset("test_mismatch")

    handle = ds.register(uris)
    result = handle.wait()  # should succeed and return 3 segment ids

    assert len(result.segment_ids) == 3
    assert set(result.segment_ids) == set(recording_ids)

    # TODO(RR-3177): we need to extend the APIs for this
    # assert result.failed_uris == [uris[2]]
    # assert "duplicate segment id" in result.something_something_error_message
