from __future__ import annotations

import os
import tempfile
from typing import TYPE_CHECKING

import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot
from rerun.catalog import AlreadyExistsError, OnDuplicateSegmentLayer, SegmentRegistrationResult

if TYPE_CHECKING:
    from collections.abc import Callable, Iterator, Sequence
    from pathlib import Path

    from rerun.catalog import CatalogClient, DatasetEntry

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


@pytest.mark.local_only
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
def test_register_unregister_batch(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test registering multiple recordings in a single call, then unregistering some of them."""
    recording_ids = [
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "cccccccc-cccc-cccc-cccc-cccccccccccc",
    ]
    uris = recording_factory(recording_ids)

    ds = entry_factory.create_dataset("test_register_unregister_batch")

    handle = ds.register(uris)
    result = handle.wait()

    assert len(result.segment_ids) == 3
    assert sorted(result.segment_ids) == sorted(recording_ids)

    df = ds.segment_table()
    assert df.count() == 3
    table = df.to_arrow_table()
    segment_ids = table.column("rerun_segment_id").to_pylist()
    assert sorted(segment_ids) == sorted(recording_ids)

    df = ds.filter_contents("/points").reader(index="log_time").sort("rerun_segment_id").drop("log_time")
    assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                             │
│ * version: 0.1.2                                                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌──────────────────────────────────────┬──────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_segment_id                     ┆ log_tick             ┆ /points:Points2D:positions                          │ │
│ │ ---                                  ┆ ---                  ┆ ---                                                 │ │
│ │ type: Utf8                           ┆ type: nullable i64   ┆ type: nullable List[nullable FixedSizeList[f32; 2]] │ │
│ │                                      ┆ index_name: log_tick ┆ archetype: Points2D                                 │ │
│ │                                      ┆ kind: index          ┆ component: Points2D:positions                       │ │
│ │                                      ┆                      ┆ component_type: Position2D                          │ │
│ │                                      ┆                      ┆ entity_path: /points                                │ │
│ │                                      ┆                      ┆ kind: data                                          │ │
│ ╞══════════════════════════════════════╪══════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa ┆ 0                    ┆ [[0.0, 0.0]]                                        │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb ┆ 0                    ┆ [[1.0, 1.0]]                                        │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ cccccccc-cccc-cccc-cccc-cccccccccccc ┆ 0                    ┆ [[2.0, 2.0]]                                        │ │
│ └──────────────────────────────────────┴──────────────────────┴─────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    ds.unregister(segments_to_drop=[recording_ids[0], recording_ids[2]], layers_to_drop=[])

    df = ds.segment_table()
    assert df.count() == 1
    table = df.to_arrow_table()
    segment_ids = table.column("rerun_segment_id").to_pylist()
    assert segment_ids == [recording_ids[1]]

    df = ds.filter_contents("/points").reader(index="log_time").sort("rerun_segment_id").drop("log_time")
    assert str(df) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                             │
│ * version: 0.1.2                                                                                                      │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌──────────────────────────────────────┬──────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_segment_id                     ┆ log_tick             ┆ /points:Points2D:positions                          │ │
│ │ ---                                  ┆ ---                  ┆ ---                                                 │ │
│ │ type: Utf8                           ┆ type: nullable i64   ┆ type: nullable List[nullable FixedSizeList[f32; 2]] │ │
│ │                                      ┆ index_name: log_tick ┆ archetype: Points2D                                 │ │
│ │                                      ┆ kind: index          ┆ component: Points2D:positions                       │ │
│ │                                      ┆                      ┆ component_type: Position2D                          │ │
│ │                                      ┆                      ┆ entity_path: /points                                │ │
│ │                                      ┆                      ┆ kind: data                                          │ │
│ ╞══════════════════════════════════════╪══════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb ┆ 0                    ┆ [[1.0, 1.0]]                                        │ │
│ └──────────────────────────────────────┴──────────────────────┴─────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


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


@pytest.mark.local_only
def test_register_conflicting_schema(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Test that two RRDs with conflicting schemas are not allowed to be registered to a dataset."""

    import pyarrow as pa

    seg_1_path = tmp_path / "segment1.rrd"
    seg_2_path = tmp_path / "segment2.rrd"

    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment1") as rec:
        rec.save(seg_1_path)
        rec.log("/data", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float64())))

    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment2") as rec:
        rec.save(seg_2_path)
        rec.log("/data", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float32())))

    dataset = entry_factory.create_dataset("test_conflicting_schema")

    with pytest.raises(ValueError, match="schema"):
        dataset.register([seg_1_path.as_uri(), seg_2_path.as_uri()]).wait()


@pytest.mark.local_only
def test_register_conflicting_property_schema(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Test that two RRDs with conflicting schemas are not allowed to be registered to a dataset."""

    import pyarrow as pa

    seg_1_path = tmp_path / "segment1.rrd"
    seg_2_path = tmp_path / "segment2.rrd"

    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment1") as rec:
        rec.save(seg_1_path)
        rec.send_property("prop", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float64())))

    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment2") as rec:
        rec.save(seg_2_path)
        rec.send_property("prop", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float32())))

    dataset = entry_factory.create_dataset("test_conflicting_property_schema")

    with pytest.raises(ValueError, match="schema"):
        dataset.register([seg_1_path.as_uri(), seg_2_path.as_uri()]).wait()


@pytest.mark.local_only
def test_failed_registration_not_in_segment_table(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Test that a failed segment registration does not show up in the segment table (separate segment id)."""

    import pyarrow as pa

    seg_1_path = tmp_path / "segment1.rrd"
    seg_2_path = tmp_path / "segment2.rrd"

    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment1") as rec:
        rec.save(seg_1_path)
        rec.send_property("prop", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float64())))

    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment2") as rec:
        rec.save(seg_2_path)
        rec.send_property("prop", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float32())))

    dataset = entry_factory.create_dataset("test_conflicting_property_schema")

    dataset.register(seg_1_path.as_uri()).wait()

    with pytest.raises(ValueError, match="schema"):
        dataset.register(seg_2_path.as_uri()).wait()

    # Verify it's segment1 (the successful one), not segment2 (the failed one)
    segment_ids = dataset.segment_ids()
    assert segment_ids == ["segment1"], f"Expected only segment1, got {segment_ids}"


@pytest.mark.local_only
def test_failed_layer_registration_not_in_segment_table(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Test that a failed segment registration does not show up in the segment table (same segment id, different layers)."""

    import pyarrow as pa

    base_path = tmp_path / "base.rrd"
    extra_path = tmp_path / "extra.rrd"

    # Both use the same recording_id (segment1) but different layer names
    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment1") as rec:
        rec.save(base_path)
        rec.send_property("prop", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float64())))

    with rr.RecordingStream("rerun_example_conflicting_schema", recording_id="segment1") as rec:
        rec.save(extra_path)
        rec.send_property("prop", rr.AnyValues(test=pa.array([1.0, 2.0, 3.0], type=pa.float32())))

    dataset = entry_factory.create_dataset("test_failed_layer_not_in_segment_table")

    # Register base layer - should succeed
    dataset.register(base_path.as_uri(), layer_name="base").wait()

    # Register extra layer with conflicting schema - should fail
    with pytest.raises(ValueError, match="schema"):
        dataset.register(extra_path.as_uri(), layer_name="extra").wait()

    # The segment table should still show the segment (because the base layer succeeded)
    df = dataset.segment_table()
    assert df.count() == 1

    # Verify segment_id and layer_names columns
    table = df.to_arrow_table()
    segment_ids = table.column("rerun_segment_id").to_pylist()
    layer_names = table.column("rerun_layer_names").to_pylist()

    assert segment_ids == ["segment1"], f"Expected segment1, got {segment_ids}"
    # Only the successful "base" layer should appear, not the failed "extra" layer
    assert layer_names == [["base"]], f"Expected [['base']], got {layer_names}"


@pytest.mark.local_only
def test_register_duplicate_error_behavior(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test that registering duplicate segments with on_duplicate='error' (default) raises an error."""
    recording_id = "88888888-8888-8888-8888-888888888888"
    uris = recording_factory([recording_id])

    ds = entry_factory.create_dataset("test_dup_error")

    # First registration should succeed
    handle = ds.register(uris[0], on_duplicate=OnDuplicateSegmentLayer.ERROR)
    result = handle.wait()
    assert len(result.segment_ids) == 1
    assert result.segment_ids[0] == recording_id

    # Second registration of the same segment should fail
    with pytest.raises(AlreadyExistsError, match="already exists"):
        ds.register(uris[0], on_duplicate=OnDuplicateSegmentLayer.ERROR).wait()


@pytest.mark.local_only
def test_register_duplicate_ignore_behavior(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test that registering duplicate segments with on_duplicate='ignore' keeps the original data."""
    recording_id = "99999999-9999-9999-9999-999999999999"
    # Create two recordings with the same ID but different data
    # uris[0] has points [[0, 0]], uris[1] has points [[1, 1]]
    uris = recording_factory([recording_id, recording_id])

    ds = entry_factory.create_dataset("test_dup_ignore")

    # First registration
    handle = ds.register(uris[0], on_duplicate=OnDuplicateSegmentLayer.SKIP)
    result = handle.wait()
    assert len(result.segment_ids) == 1
    assert result.segment_ids[0] == recording_id

    # Verify the first recording's data is present (points [[0, 0]])
    points = _get_points_data(ds)
    assert points == [[0.0, 0.0]], f"Expected [[0.0, 0.0]] but got {points}"

    # Second registration should succeed but not replace the data
    handle = ds.register(uris[1], on_duplicate=OnDuplicateSegmentLayer.SKIP)
    result = handle.wait()
    # The result still contains the segment_id even though it was skipped
    assert len(result.segment_ids) == 1

    # Verify only one segment exists
    segment_ids = ds.segment_ids()
    assert len(segment_ids) == 1
    assert segment_ids[0] == recording_id

    # Verify the data is still from the first registration (points [[0, 0]])
    points = _get_points_data(ds)
    assert points == [[0.0, 0.0]], f"Expected [[0.0, 0.0]] (original data) but got {points}"


@pytest.mark.local_only
def test_register_duplicate_replace_behavior(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test that registering duplicate segments with on_duplicate='replace' replaces the original data."""
    recording_id = "aaaabbbb-aaaa-bbbb-aaaa-bbbbaaaabbbb"
    # Create two recordings with the same ID but different data
    # uris[0] has points [[0, 0]], uris[1] has points [[1, 1]]
    uris = recording_factory([recording_id, recording_id])

    ds = entry_factory.create_dataset("test_dup_replace")

    # First registration
    handle = ds.register(uris[0], on_duplicate=OnDuplicateSegmentLayer.REPLACE)
    result = handle.wait()
    assert len(result.segment_ids) == 1
    assert result.segment_ids[0] == recording_id

    # Verify the first recording's data is present (points [[0, 0]])
    points = _get_points_data(ds)
    assert points == [[0.0, 0.0]], f"Expected [[0.0, 0.0]] but got {points}"

    # Second registration should succeed and replace the data
    handle = ds.register(uris[1], on_duplicate=OnDuplicateSegmentLayer.REPLACE)
    result = handle.wait()
    assert len(result.segment_ids) == 1

    # Verify only one segment exists (not duplicated)
    segment_ids = ds.segment_ids()
    assert len(segment_ids) == 1
    assert segment_ids[0] == recording_id

    # Verify the data is now from the second registration (points [[1, 1]])
    points = _get_points_data(ds)
    assert points == [[1.0, 1.0]], f"Expected [[1.0, 1.0]] (replaced data) but got {points}"


@pytest.mark.local_only
def test_register_intra_request_duplicates(
    entry_factory: EntryFactory,
    recording_factory: Callable[[Sequence[str]], list[str]],
) -> None:
    """Test that intra-request duplicates (same segment in one call) always fail, regardless of on_duplicate mode."""
    recording_id = "ccccdddd-cccc-dddd-cccc-ddddccccdddd"
    uris = recording_factory([recording_id, recording_id])

    for on_duplicate in OnDuplicateSegmentLayer:
        ds = entry_factory.create_dataset(f"test_intra_dup_{on_duplicate.value}")

        with pytest.raises(ValueError, match="duplicate segment layers in request") as exc_info:
            ds.register(uris, on_duplicate=on_duplicate)

        error_message = str(exc_info.value)
        for uri in uris:
            assert uri in error_message, f"Expected URI {uri} in error message: {error_message}"


def _get_points_data(ds: DatasetEntry) -> list[list[float]]:
    """Helper to extract points data from a dataset."""
    import pyarrow as pa

    batches = ds.reader(index="log_time").select("/points:Points2D:positions").collect()
    table = pa.Table.from_batches(batches)
    positions_column = table.column("/points:Points2D:positions")
    # Extract all point coordinates from the nested list structure
    # The structure is: list of rows, each row is a list of points, each point is [x, y]
    points = []
    for chunk in positions_column.chunks:
        for row in chunk:
            if row is not None:
                for point in row.as_py():
                    points.append(point)
    return points
