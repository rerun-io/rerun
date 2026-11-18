"""Create, edit, and query annotated time spans."""

from __future__ import annotations

import tempfile
from pathlib import Path

import rerun as rr


def main() -> None:
    with tempfile.TemporaryDirectory() as directory:
        paths = []
        for segment_id in ("episode_1", "episode_2"):
            path = Path(directory) / f"{segment_id}.rrd"
            with rr.RecordingStream(
                "rerun_example_spans", recording_id=segment_id
            ) as recording:
                recording.save(path)
                recording.send_columns(
                    "value",
                    indexes=[rr.TimeColumn("time", duration=[0.0, 10.0])],
                    columns=rr.Scalars.columns(scalars=[0.0, 1.0]),
                )
            paths.append(path)

        with rr.server.Server(
            host="127.0.0.1", datasets={"robot_recordings": paths}
        ) as server:
            catalog = server.client()

            # region: create
            import pyarrow as pa

            from rerun.experimental.spans import SpanTable

            dataset = catalog.get_dataset("robot_recordings")
            annotations = SpanTable.create(
                dataset,
                index="time",
                schema=pa.schema([
                    ("state", pa.string()),
                    ("score", pa.float32()),
                ]),
            )
            # endregion: create

            assert SpanTable.open(dataset).table.id == annotations.table.id

            # region: add_or_edit
            from datetime import timedelta

            annotations = SpanTable.open(dataset)

            span_id = annotations.append(
                segment_id="episode_1",
                start=timedelta(seconds=1),
                end=timedelta(seconds=3),
                data={"state": "grasp", "score": 0.92},
            )

            annotations.upsert(
                segment_id="episode_1",
                span_id=span_id,
                start=timedelta(seconds=1),
                end=timedelta(seconds=3),
                data={"state": "grasp", "score": 0.97},
            )
            # endregion: add_or_edit

            rows = annotations.table.reader().to_arrow_table()
            assert rows["rerun_span_id"].to_pylist() == [span_id]
            assert rows["state"].to_pylist() == ["grasp"]
            assert round(rows["score"][0].as_py(), 2) == 0.97
            assert rows["time:start"].cast(pa.int64()).to_pylist() == [
                1_000_000_000
            ]
            assert rows["time:end"].cast(pa.int64()).to_pylist() == [
                3_000_000_000
            ]

            annotations.append(
                segment_id="episode_2",
                start=timedelta(seconds=1),
                end=timedelta(seconds=3),
                data={"state": "grasp"},
            )

            # region: query
            from datafusion import col

            annotations = SpanTable.open(dataset)
            selected = annotations.reader().filter(
                (col("state") != "failure") & (col("score") >= 0.7)
            )
            curated = selected.to_arrow_table()
            # endregion: query

            assert curated["rerun_span_id"].to_pylist() == [span_id]

            # region: filter_segment
            annotations = SpanTable.open(dataset)
            episode_spans = annotations.reader().filter(
                col("rerun_segment_id") == "episode_1"
            )
            # endregion: filter_segment

            assert episode_spans.to_arrow_table()[
                "rerun_span_id"
            ].to_pylist() == [span_id]
            assert annotations.reader().count() == 2

            # region: hierarchy
            annotations = SpanTable.open(dataset)

            annotations.append(
                segment_id="episode_1",
                start=timedelta(seconds=0),
                end=timedelta(seconds=5),
                data={"state": "pick_and_place"},
            )

            spans = annotations.reader()
            grasps = spans.filter(col("state") == "grasp").alias("grasp")
            tasks = spans.filter(col("state") == "pick_and_place").alias("task")

            nested_grasps = grasps.join_on(
                tasks,
                grasps.col("rerun_segment_id") == tasks.col("rerun_segment_id"),
                grasps.col("time:start") >= tasks.col("time:start"),
                grasps.col("time:end") <= tasks.col("time:end"),
                how="semi",
            ).to_arrow_table()
            # endregion: hierarchy

            assert nested_grasps["rerun_span_id"].to_pylist() == [span_id]

            # region: delete
            annotations = SpanTable.open(dataset)
            annotations.delete()
            # endregion: delete

            assert catalog.get_dataset("robot_recordings").id == dataset.id
            assert sorted(dataset.segment_ids()) == ["episode_1", "episode_2"]
            assert all(path.is_file() for path in paths)
            try:
                SpanTable.open(dataset)
            except LookupError:
                pass
            else:
                raise AssertionError(
                    "Deleting annotations must remove their span table"
                )


if __name__ == "__main__":
    main()
