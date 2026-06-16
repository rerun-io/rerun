"""Tests for `DatasetEntry._register_asset_layer`."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.catalog import AlreadyExistsError, OnDuplicateSegmentLayer

pytestmark = [
    pytest.mark.filterwarnings("ignore:_register_asset_layer:UserWarning"),
]

if TYPE_CHECKING:
    from pathlib import Path

    from e2e_redap_tests.conftest import EntryFactory


def _make_recording(path: Path, recording_id: str) -> str:
    """Write a minimal RRD and return its file URI."""
    with rr.RecordingStream("rerun_example_test_asset_layer", recording_id=recording_id) as rec:
        rec.set_log_tick_enabled(True)
        rec.save(path)
        rec.log("asset", rr.Points2D([[1.0, 2.0]]))
        rec.flush()
    return path.absolute().as_uri()


@pytest.mark.local_only  # TODO(RR-4761): implement on Hub
def test_register_asset_layer_basic(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Register a single RRD as an asset layer and verify the handle can be awaited."""
    recording_id = "aaaabbbb-cccc-dddd-eeee-ffffffffffff"
    uri = _make_recording(tmp_path / "asset.rrd", recording_id)

    ds = entry_factory.create_dataset("test_asset_layer_basic")

    handle = ds._register_asset_layer(layer_name="robot_urdf", recording_uri=uri)
    result = handle.wait()

    assert len(result.segment_ids) == 1
    assert result.segment_ids[0] == recording_id


@pytest.mark.local_only  # TODO(RR-4761): implement on Hub
def test_register_asset_layer_appears_in_manifest(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """
    Asset layer shows up in the manifest once per segment.

    TODO(RR-4807): consider this choice — an asset layer in a segment-less dataset
    is invisible in the manifest.
    """
    asset_id = "11112222-3333-4444-5555-666677778888"
    seg_id = "99998888-7777-6666-5555-444433332222"

    asset_uri = _make_recording(tmp_path / "asset.rrd", asset_id)
    seg_uri = _make_recording(tmp_path / "segment.rrd", seg_id)

    ds = entry_factory.create_dataset("test_asset_layer_manifest")

    ds._register_asset_layer(layer_name="my_asset", recording_uri=asset_uri).wait()

    # No segments yet, so the asset layer is invisible in the manifest.
    table = ds._manifest(include_diagnostic_data=False).to_arrow_table()
    assert table.num_rows == 0

    # Register a segment; the asset layer should now appear for it.
    ds.register([seg_uri]).wait()

    table = ds._manifest(include_diagnostic_data=False).to_arrow_table()
    layer_names = table.column("rerun_layer_name").to_pylist()
    assert "my_asset" in layer_names
    assert "base" in layer_names


@pytest.mark.local_only  # TODO(RR-4761): implement on Hub
def test_register_asset_layer_with_segment_layers(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Asset layer and regular segment layers can coexist in the same dataset."""
    asset_id = "aaaaaaaa-0000-0000-0000-000000000000"
    seg_id = "bbbbbbbb-1111-1111-1111-111111111111"

    asset_uri = _make_recording(tmp_path / "asset.rrd", asset_id)
    seg_uri = _make_recording(tmp_path / "segment.rrd", seg_id)

    ds = entry_factory.create_dataset("test_asset_and_segment_layers")

    ds._register_asset_layer(layer_name="shared_mesh", recording_uri=asset_uri).wait()
    ds.register([seg_uri]).wait()

    manifest = ds._manifest(include_diagnostic_data=False)
    table = manifest.to_arrow_table()

    layer_names = set(table.column("rerun_layer_name").to_pylist())
    assert "shared_mesh" in layer_names
    assert "base" in layer_names


@pytest.mark.local_only  # TODO(RR-4761): implement on Hub
def test_register_asset_layer_duplicate_error(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Re-registering the same asset layer with on_duplicate=ERROR raises AlreadyExistsError."""
    recording_id = "cccccccc-cccc-cccc-cccc-cccccccccccc"
    uri = _make_recording(tmp_path / "asset.rrd", recording_id)

    ds = entry_factory.create_dataset("test_asset_layer_dup_error")

    ds._register_asset_layer(
        layer_name="robot_urdf", recording_uri=uri, on_duplicate=OnDuplicateSegmentLayer.ERROR
    ).wait()

    with pytest.raises(AlreadyExistsError, match="already exists"):
        ds._register_asset_layer(
            layer_name="robot_urdf", recording_uri=uri, on_duplicate=OnDuplicateSegmentLayer.ERROR
        ).wait()


@pytest.mark.local_only  # TODO(RR-4761): implement on Hub
def test_register_asset_layer_duplicate_replace(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Re-registering with on_duplicate=REPLACE succeeds."""
    asset_id = "dddddddd-dddd-dddd-dddd-dddddddddddd"
    seg_id = "dddddddd-1111-1111-1111-111111111111"

    asset_uri = _make_recording(tmp_path / "asset.rrd", asset_id)
    seg_uri = _make_recording(tmp_path / "segment.rrd", seg_id)

    ds = entry_factory.create_dataset("test_asset_layer_dup_replace")
    ds.register([seg_uri]).wait()

    ds._register_asset_layer(
        layer_name="robot_urdf", recording_uri=asset_uri, on_duplicate=OnDuplicateSegmentLayer.REPLACE
    ).wait()
    ds._register_asset_layer(
        layer_name="robot_urdf", recording_uri=asset_uri, on_duplicate=OnDuplicateSegmentLayer.REPLACE
    ).wait()

    manifest = ds._manifest(include_diagnostic_data=False)
    layer_names = manifest.to_arrow_table().column("rerun_layer_name").to_pylist()
    assert layer_names.count("robot_urdf") == 1


@pytest.mark.local_only  # TODO(RR-4761): implement on Hub
def test_register_asset_layer_duplicate_skip(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """Re-registering with on_duplicate=SKIP succeeds and leaves the manifest unchanged."""
    asset_id = "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee"
    seg_id = "eeeeeeee-1111-1111-1111-111111111111"

    asset_uri = _make_recording(tmp_path / "asset.rrd", asset_id)
    seg_uri = _make_recording(tmp_path / "segment.rrd", seg_id)

    ds = entry_factory.create_dataset("test_asset_layer_dup_skip")
    ds.register([seg_uri]).wait()

    ds._register_asset_layer(
        layer_name="robot_urdf", recording_uri=asset_uri, on_duplicate=OnDuplicateSegmentLayer.SKIP
    ).wait()
    ds._register_asset_layer(
        layer_name="robot_urdf", recording_uri=asset_uri, on_duplicate=OnDuplicateSegmentLayer.SKIP
    ).wait()

    manifest = ds._manifest(include_diagnostic_data=False)
    layer_names = manifest.to_arrow_table().column("rerun_layer_name").to_pylist()
    assert layer_names.count("robot_urdf") == 1
