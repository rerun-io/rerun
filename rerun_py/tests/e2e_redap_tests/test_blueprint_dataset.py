from __future__ import annotations

from typing import TYPE_CHECKING

import rerun as rr
import rerun.blueprint as rrb

if TYPE_CHECKING:
    from pathlib import Path


def test_configure_blueprint_dataset(catalog_client: rr.catalog.CatalogClient, tmp_path: Path) -> None:
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

    # Create a new dataset
    ds = catalog_client.create_dataset("my_new_dataset")

    # Register our recording to the dataset
    ds.register(rrd_path.absolute().as_uri())

    # Register our blueprint to the corresponding blueprint dataset
    bds = ds.blueprint_dataset()
    assert bds is not None

    blueprint_partition_id = bds.register(rbl_path.absolute().as_uri())

    # Set our newly registered blueprint as default for our dataset
    ds.set_default_blueprint_partition_id(blueprint_partition_id)

    # Uncomment this line for a chance to connect to this server using the viewer
    # input(f"Server running on {server.address()}. Press enter to continue…"

    assert ds.default_blueprint_partition_id() == blueprint_partition_id
