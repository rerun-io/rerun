from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
import rerun.blueprint as rrb

if TYPE_CHECKING:
    from pathlib import Path

    from e2e_redap_tests.conftest import EntryFactory


@pytest.mark.local_only
def test_configure_blueprint_dataset(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """
    Test configuring a blueprint dataset.

    This test is marked as local_only because it uses RecordingStream to generate
    .rrd and .rbl files on-the-fly, which cannot be used with remote deployments.
    """
    # Create a recording and save it to a temporary file
    rrd_path = tmp_path / "recording.rrd"
    rec = rr.RecordingStream("rerun_example_dataset_blueprint")
    rec.save(rrd_path)
    rec.log("points", rr.Points2D([[0, 0], [1, 1]]))
    rec.flush()

    # Create a blueprint and save it to a temporary file
    rbl_path = tmp_path / "blueprint.rbl"
    blueprint = rrb.Blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=[-1, 2], y_range=[-1, 2])))
    blueprint.save("rerun_example_dataset_blueprint", rbl_path)

    # Create another blueprint
    rbl_path2 = tmp_path / "blueprint2.rbl"
    blueprint = rrb.Blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=[-1, 2], y_range=[-1, 2])))
    blueprint.save("rerun_example_dataset_blueprint", rbl_path2)

    # Create a new dataset
    ds = entry_factory.create_dataset("my_new_dataset")

    # Register our recording to the dataset
    ds.register(rrd_path.absolute().as_uri()).wait()

    # Register our blueprint to the corresponding blueprint dataset
    bds = ds.blueprint_dataset()
    assert bds is not None

    # Register first blueprint
    ds.register_blueprint(rbl_path.absolute().as_uri())

    assert len(bds.segment_ids()) == 1
    first_blueprint_name = ds.default_blueprint()

    # Register the second blueprint
    ds.register_blueprint(rbl_path2.absolute().as_uri(), set_default=False)

    assert len(bds.segment_ids()) == 2
    assert first_blueprint_name == ds.default_blueprint()

    # Get the second blueprint name
    [second_blueprint_name] = list(set(bds.segment_ids()) - {first_blueprint_name})

    ds.set_default_blueprint(second_blueprint_name)
    assert second_blueprint_name == ds.default_blueprint()
