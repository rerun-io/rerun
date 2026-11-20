"""Query recordings on a catalog server and convert the results."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import numpy as np

import rerun as rr

NUM_FRAMES = 150


def write_recording(path: Path, recording_id: str) -> None:
    """Write a robot recording on a `frame_nr` timeline."""
    with rr.RecordingStream(
        "rerun_example_get_data_out", recording_id=recording_id
    ) as recording:
        recording.save(path)
        frame_nr = rr.TimeColumn("frame_nr", sequence=range(NUM_FRAMES))
        positions = np.linspace(0.0, 1.0, NUM_FRAMES * 3).reshape(-1, 3)
        for entity in ("/world/robot", "/world/robot/arm"):
            recording.send_columns(
                entity,
                indexes=[frame_nr],
                columns=rr.Points3D.columns(positions=positions),
            )
        recording.send_columns(
            "/world/robot/debug",
            indexes=[frame_nr],
            columns=rr.TextLog.columns(text=["ok"] * NUM_FRAMES),
        )
        for entity in ("/sensors/imu", "/robot/gripper/force"):
            recording.send_columns(
                entity,
                indexes=[frame_nr],
                columns=rr.Scalars.columns(
                    scalars=np.sin(np.arange(NUM_FRAMES) / 10.0)
                ),
            )


def main() -> None:
    previous_dir = Path.cwd()
    with tempfile.TemporaryDirectory() as directory:
        os.chdir(directory)
        try:
            write_recording(Path("recording.rrd"), "recording_001")
            write_recording(Path("recording1.rrd"), "recording_001")
            write_recording(Path("recording2.rrd"), "recording_002")
            Path("recordings_dir").mkdir()
            write_recording(
                Path("recordings_dir") / "recording3.rrd", "recording_003"
            )
            run_queries()
            convert_to_arrow()
            convert_to_pandas()
            convert_to_polars()
            read_without_server()
        finally:
            os.chdir(previous_dir)


def run_queries() -> None:
    # region: start_server
    import rerun as rr

    # Start a server with one or more .rrd files
    with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
        client = server.client()
        dataset = client.get_dataset("my_dataset")
    # endregion: start_server

    # region: multiple_datasets
    with rr.server.Server(
        datasets={
            # Explicit list of RRD files
            "dataset1": ["recording1.rrd", "recording2.rrd"],
            # Directory containing RRD files
            "dataset2": "recordings_dir",
        }
    ) as server:
        client = server.client()
        # Access each dataset by name
        ds1 = client.get_dataset("dataset1")
        ds2 = client.get_dataset("dataset2")
        # endregion: multiple_datasets

        assert sorted(ds1.segment_ids()) == ["recording_001", "recording_002"]
        assert ds2.segment_ids() == ["recording_003"]
        dataset = ds1

        # region: filter_selection
        # Filter by entity paths
        selection = dataset.filter_contents(["/world/robot/**", "/sensors/**"])

        # Filter by segment IDs
        selection = dataset.filter_segments(["recording_001", "recording_002"])

        # Chain filters
        selection = dataset.filter_contents(["/world/**"]).filter_segments([
            "recording_001"
        ])
        # endregion: filter_selection

        assert selection.reader(index="frame_nr").count() == NUM_FRAMES

        # region: filter_semantics
        from rerun.catalog import ContentFilter

        # Selects `/world/robot/arm/**` only
        selection = dataset.filter_contents("/world/robot/**").filter_contents(
            "/world/robot/arm/**"
        )

        # Selects nothing, since no entity is in both subtrees
        selection = dataset.filter_contents("/world/robot/**").filter_contents(
            "/sensors/**"
        )

        # Selects both subtrees
        selection = dataset.filter_contents(["/world/robot/**", "/sensors/**"])

        # Selects `/world/robot/**` without `/world/robot/debug/**`
        selection = dataset.filter_contents([
            "/world/robot/**",
            "-/world/robot/debug/**",
        ])

        # Same, but narrowing an existing selection
        robot = dataset.filter_contents("/world/robot/**")
        selection = robot.filter_contents(
            ContentFilter.everything().exclude("/world/robot/debug/**")
        )
        # endregion: filter_semantics

        columns = selection.reader(index="frame_nr").schema().names
        assert "/world/robot/arm:Points3D:positions" in columns
        assert "/world/robot/debug:TextLog:text" not in columns

        # region: datafusion_filter
        from datafusion import col

        df = dataset.reader(index="frame_nr")

        # Filter by index range
        df = df.filter(col("frame_nr") >= 0).filter(col("frame_nr") <= 100)

        # Filter by column not null
        df = df.filter(col("/world/robot:Points3D:positions").is_not_null())

        # Select specific columns
        df = df.select("frame_nr", "/world/robot:Points3D:positions")
        # endregion: datafusion_filter

        assert df.count() == 2 * 101

        for name in ("run_a", "run_b"):
            client.create_dataset(name).register([
                Path("recording1.rrd").resolve().as_uri()
            ]).wait()

        # region: cross_datasets
        ctx = client.ctx
        ctx.register_view(
            "run_a", client.get_dataset("run_a").reader(index="frame_nr")
        )
        ctx.register_view(
            "run_b", client.get_dataset("run_b").reader(index="frame_nr")
        )

        df = ctx.sql("""
            SELECT rerun_segment_id, frame_nr,
                "/robot/gripper/force:Scalars:scalars"[1] AS force
            FROM run_a
            UNION ALL
            SELECT rerun_segment_id, frame_nr,
                "/robot/gripper/force:Scalars:scalars"[1] AS force
            FROM run_b
        """)
        # endregion: cross_datasets

        assert df.count() == 2 * NUM_FRAMES


def convert_to_arrow() -> None:
    # region: to_arrow
    import rerun as rr

    with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
        dataset = server.client().get_dataset("my_dataset")
        table = dataset.reader(index="frame_nr").to_arrow_table()
    # endregion: to_arrow

    assert table.num_rows == NUM_FRAMES


def convert_to_pandas() -> None:
    # region: to_pandas
    import rerun as rr

    with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
        dataset = server.client().get_dataset("my_dataset")
        df = dataset.reader(index="frame_nr").to_pandas()
    # endregion: to_pandas

    assert len(df) == NUM_FRAMES


def convert_to_polars() -> None:
    # region: to_polars
    import polars as pl

    import rerun as rr

    with rr.server.Server(datasets={"my_dataset": ["recording.rrd"]}) as server:
        dataset = server.client().get_dataset("my_dataset")
        df = pl.from_arrow(dataset.reader(index="frame_nr").to_arrow_table())
    # endregion: to_polars

    assert len(df) == NUM_FRAMES


def read_without_server() -> None:
    # region: rrd_reader
    from rerun.chunk import RrdReader

    store = RrdReader("recording.rrd").stream().collect()
    df = store.reader(index="frame_nr")

    print(df.to_pandas())
    # endregion: rrd_reader


if __name__ == "__main__":
    main()
