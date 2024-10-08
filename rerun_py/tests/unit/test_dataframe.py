from __future__ import annotations

import tempfile
import uuid

import pyarrow as pa
import rerun as rr

APP_ID = "rerun_example_test_recording"
RECORDING_ID = uuid.uuid4()


class TestDataframe:
    def setup_method(self) -> None:
        rr.init(APP_ID, recording_id=RECORDING_ID)

        rr.set_time_sequence("my_index", 1)
        rr.log("points", rr.Points3D([[1, 2, 3], [4, 5, 6], [7, 8, 9]]))
        rr.set_time_sequence("my_index", 7)
        rr.log("points", rr.Points3D([[10, 11, 12]], colors=[[255, 0, 0]]))

        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/tmp.rrd"

            rr.save(rrd)

            self.recording = rr.dataframe.load_recording(rrd)

    def test_recording_info(self) -> None:
        assert self.recording.application_id() == APP_ID
        assert self.recording.recording_id() == str(RECORDING_ID)

    def test_full_view(self) -> None:
        view = self.recording.view(index="my_index", contents="points")

        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, indicator, points, colors
        assert table.num_columns == 6
        assert table.num_rows == 2

    def test_select_columns(self) -> None:
        view = self.recording.view(index="my_index", contents="points")
        log_time = rr.dataframe.IndexColumnSelector("my_index")
        pos = rr.dataframe.ComponentColumnSelector("points", rr.components.Position3D)

        batches = view.select(log_time, pos)

        table = pa.Table.from_batches(batches, batches.schema)
        # points
        assert table.num_columns == 2
        assert table.num_rows == 2

        expected_index0 = pa.array(
            [1],
            type=pa.int64(),
        )

        expected_index1 = pa.array(
            [7],
            type=pa.int64(),
        )

        expected_pos0 = pa.array(
            [
                [1, 2, 3],
                [4, 5, 6],
                [7, 8, 9],
            ],
            type=rr.components.Position3D.arrow_type(),
        )

        expected_pos1 = pa.array(
            [
                [10, 11, 12],
            ],
            type=rr.components.Position3D.arrow_type(),
        )

        print(table.schema)

        assert table.column("my_index")[0].equals(expected_index0[0])
        assert table.column("my_index")[1].equals(expected_index1[0])
        assert table.column("/points:Position3D")[0].values.equals(expected_pos0)
        assert table.column("/points:Position3D")[1].values.equals(expected_pos1)

    def test_view_syntax(self) -> None:
        good_content_expressions = [
            {"points": rr.components.Position3D},
            {"points": [rr.components.Position3D]},
            {"points": "rerun.components.Position3D"},
            {"points/**": "rerun.components.Position3D"},
        ]

        for expr in good_content_expressions:
            view = self.recording.view(index="my_index", contents=expr)
            batches = view.select()
            table = pa.Table.from_batches(batches, batches.schema)

            # my_index, log_time, log_tick, points
            assert table.num_columns == 4
            assert table.num_rows == 2

        bad_content_expressions = [
            {"points": rr.components.Position2D},
            {"point": [rr.components.Position3D]},
        ]

        for expr in bad_content_expressions:
            view = self.recording.view(index="my_index", contents=expr)
            batches = view.select()

            # my_index, log_time, log_tick
            table = pa.Table.from_batches(batches, batches.schema)
            assert table.num_columns == 3
            assert table.num_rows == 0
