"""Inspect segment layers, schemas, and assets of a catalog dataset."""

from __future__ import annotations

import tempfile
from pathlib import Path

import rerun as rr


def write_rrd(path: Path, recording_id: str, entity: str) -> str:
    with rr.RecordingStream(
        "rerun_example_catalog_object_model", recording_id=recording_id
    ) as recording:
        recording.save(path)
        recording.send_columns(
            entity,
            indexes=[rr.TimeColumn("frame", sequence=[0, 1, 2])],
            columns=rr.Scalars.columns(scalars=[0.0, 0.5, 1.0]),
        )
    return path.resolve().as_uri()


def write_static_rrd(path: Path) -> str:
    with rr.RecordingStream(
        "rerun_example_catalog_object_model", recording_id="robot_mesh"
    ) as recording:
        recording.save(path)
        recording.log(
            "/world/robot/mesh",
            rr.Points3D([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]),
            static=True,
        )
    return path.resolve().as_uri()


def main() -> None:
    with tempfile.TemporaryDirectory() as directory:
        tmp = Path(directory)
        base_a = write_rrd(tmp / "a_base.rrd", "segment_a", "/robot/joint")
        predictions_a = write_rrd(
            tmp / "a_predictions.rrd", "segment_a", "/robot/prediction"
        )
        base_b = write_rrd(tmp / "b_base.rrd", "segment_b", "/robot/joint")
        robot_mesh_uri = write_static_rrd(tmp / "robot_mesh.rrd")

        with rr.server.Server() as server:
            client = rr.catalog.CatalogClient(server.url())
            dataset = client.create_dataset("my_dataset")
            dataset.register(
                [base_a, base_b, predictions_a],
                layer_name=["base", "base", "predictions"],
            ).wait()
            segment_id = "segment_a"

            # region: schema
            segment_schema = dataset.segment(segment_id).schema()
            print(segment_schema)
            print(dataset.layer("base").schema())
            print(dataset.schema())
            print(dataset.schema().to_arrow())
            # endregion: schema

            # region: segment_layers
            segment = dataset.segment(segment_id)
            print(segment.layers())
            print(segment.url)

            unregistration = segment.delete_layer("predictions")
            unregistration.wait()
            # endregion: segment_layers

            assert set(dataset.segment(segment_id).layers()) == {"base"}

            # region: assets
            dataset = client.get_dataset("my_dataset")

            # e.g. "s3://bucket/robot_mesh.rrd"
            asset_id = dataset.register_asset(robot_mesh_uri)
            print(dataset.assets())

            dataset.unregister_asset(asset_id)
            # endregion: assets

            assert dataset.assets() == []
            dataset.register_asset(robot_mesh_uri)

            # region: segment_store
            segment_id = dataset.segment_ids()[0]

            # Covers the chunks of both the segment and the dataset's assets.
            store = dataset.segment_store(segment_id)
            for chunk in store.stream().to_chunks():
                print(chunk.entity_path)

            # Covers only the segment, and no asset manifests are fetched.
            segment_only = dataset.segment_store(
                segment_id, include_assets=False
            )
            # endregion: segment_store

            assert segment_only is not None

            # region: asset_dataset
            asset_dataset = dataset.asset_dataset()
            assert asset_dataset is not None

            print(asset_dataset.schema())

            # Assets only hold static data, so there is no index to read along.
            df = asset_dataset.reader(index=None)
            # endregion: asset_dataset

            assert df.count() == 1


if __name__ == "__main__":
    main()
