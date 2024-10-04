from __future__ import annotations

import tempfile

import pyarrow as pa
import rerun as rr


class TestDataframe:
    def setup_method(self):
        rr.init("rerun_example_test_recording")

        rr.log("points", rr.Points3D([[1, 2, 3], [4, 5, 6], [7, 8, 9]]))
        rr.log("points", rr.Points3D([[10, 11, 12]], colors=[[255, 0, 0]]))

        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/tmp.rrd"

            rr.save(rrd)

            self.recording = rr.dataframe.load_recording(rrd)

    def test_full_view(self) -> None:
        view = self.recording.view(index="log_time", contents="points")

        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # row, log_time, log_tick, indicator, points, colors
        assert table.num_columns == 6
        assert table.num_rows == 2

    def test_select_column(self) -> None:
        view = self.recording.view(index="log_time", contents="points")
        pos = rr.dataframe.ComponentColumnSelector("points", rr.components.Position3D)
        batches = view.select(pos)

        table = pa.Table.from_batches(batches, batches.schema)
        # points
        assert table.num_columns == 1
        assert table.num_rows == 2

        expected0 = pa.array(
            [
                [1, 2, 3],
                [4, 5, 6],
                [7, 8, 9],
            ],
            type=rr.components.Position3D.arrow_type(),
        )

        expected1 = pa.array(
            [
                [10, 11, 12],
            ],
            type=rr.components.Position3D.arrow_type(),
        )

        assert table.column(0)[0].values.equals(expected0)
        assert table.column(0)[1].values.equals(expected1)

    def test_view_syntax(self) -> None:
        good_content_expressions = [
            {"points": rr.components.Position3D},
            {"points": [rr.components.Position3D]},
            {"points": "rerun.components.Position3D"},
            {"points/**": "rerun.components.Position3D"},
        ]

        for expr in good_content_expressions:
            view = self.recording.view(index="log_time", contents=expr)
            batches = view.select()
            table = pa.Table.from_batches(batches, batches.schema)

            # row, log_time, log_tick, points
            assert table.num_columns == 4
            assert table.num_rows == 2

        bad_content_expressions = [
            {"points": rr.components.Position2D},
            {"point": [rr.components.Position3D]},
        ]

        for expr in bad_content_expressions:
            view = self.recording.view(index="log_time", contents=expr)
            batches = view.select()
            print(batches)
            table = pa.Table.from_batches(batches, batches.schema)
            assert table.num_columns == 3
            assert table.num_rows == 0
