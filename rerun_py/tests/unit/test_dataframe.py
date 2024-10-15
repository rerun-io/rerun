from __future__ import annotations

import pathlib
import tempfile
import uuid

import pyarrow as pa
import pytest
import rerun as rr

APP_ID = "rerun_example_test_recording"
RECORDING_ID = uuid.uuid4()


def test_load_recording() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rrd = tmpdir + "/tmp.rrd"

        rr.init("rerun_example_test_recording")
        rr.set_time_sequence("my_index", 1)
        rr.log("log", rr.TextLog("Hello"))
        rr.save(rrd)

        recording = rr.dataframe.load_recording(rrd)
        assert recording is not None

        view = recording.view(index="my_index", contents="/**")
        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, text
        assert table.num_columns == 4
        assert table.num_rows == 1

        recording = rr.dataframe.load_recording(pathlib.Path(tmpdir) / "tmp.rrd")
        assert recording is not None

        view = recording.view(index="my_index", contents="/**")
        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, text
        assert table.num_columns == 4
        assert table.num_rows == 1


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

        self.expected_index0 = pa.array(
            [1],
            type=pa.int64(),
        )

        self.expected_index1 = pa.array(
            [7],
            type=pa.int64(),
        )

        self.expected_pos0 = pa.array(
            [
                [1, 2, 3],
                [4, 5, 6],
                [7, 8, 9],
            ],
            type=rr.components.Position3D.arrow_type(),
        )

        self.expected_pos1 = pa.array(
            [
                [10, 11, 12],
            ],
            type=rr.components.Position3D.arrow_type(),
        )

        self.empty_pos = pa.array(
            [],
            type=rr.components.Position3D.arrow_type(),
        )

    def test_recording_info(self) -> None:
        assert self.recording.application_id() == APP_ID
        assert self.recording.recording_id() == str(RECORDING_ID)

    def test_schema(self) -> None:
        schema = self.recording.schema()

        assert len(schema.index_columns()) == 3
        # Points3DIndicator, Position3D, Color
        assert len(schema.component_columns()) == 3

        assert schema.index_columns()[0].name == "log_tick"
        assert schema.index_columns()[1].name == "log_time"
        assert schema.index_columns()[2].name == "my_index"
        assert schema.component_columns()[0].entity_path == "/points"
        assert schema.component_columns()[0].component_name == "rerun.components.Color"
        assert schema.component_columns()[1].entity_path == "/points"
        assert schema.component_columns()[1].component_name == "rerun.components.Points3DIndicator"
        assert schema.component_columns()[2].entity_path == "/points"
        assert schema.component_columns()[2].component_name == "rerun.components.Position3D"

    def test_full_view(self) -> None:
        view = self.recording.view(index="my_index", contents="points")

        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 2

    def test_select_columns(self) -> None:
        view = self.recording.view(index="my_index", contents="points")
        index_col = rr.dataframe.IndexColumnSelector("my_index")

        selectors = [rr.components.Position3D, "rerun.components.Position3D", "Position3D", "position3D"]
        for selector in selectors:
            pos = rr.dataframe.ComponentColumnSelector("points", selector)

            batches = view.select(index_col, pos)

            table = pa.Table.from_batches(batches, batches.schema)
            # points
            assert table.num_columns == 2
            assert table.num_rows == 2

            assert table.column("my_index")[0].equals(self.expected_index0[0])
            assert table.column("my_index")[1].equals(self.expected_index1[0])
            assert table.column("/points:Position3D")[0].values.equals(self.expected_pos0)
            assert table.column("/points:Position3D")[1].values.equals(self.expected_pos1)

    def test_index_values(self) -> None:
        view = self.recording.view(index="my_index", contents="points")
        view = view.filter_index_values([1, 7, 9])

        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 2

        assert table.column("my_index")[0].equals(self.expected_index0[0])
        assert table.column("my_index")[1].equals(self.expected_index1[0])

        # This is a chunked array
        new_selection_chunked = table.column("my_index").take([1])

        # This is a single array
        new_selection = new_selection_chunked.combine_chunks()

        view2 = view.filter_index_values(new_selection_chunked)
        batches = view2.select()
        table2 = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, points, colors
        assert table2.num_columns == 5
        assert table2.num_rows == 1

        assert table2.column("my_index")[0].equals(self.expected_index1[0])

        view3 = view.filter_index_values(new_selection)
        batches = view3.select()
        table3 = pa.Table.from_batches(batches, batches.schema)

        assert table3 == table2

        # Manually create a pyarrow array with no matches
        view4 = view.filter_index_values(pa.array([8], type=pa.int64()))
        batches = view4.select()
        table4 = pa.Table.from_batches(batches, batches.schema)

        assert table4.num_rows == 0

        # Manually create a chunked array with 1 match
        manual_chunked_selection = pa.chunked_array([
            pa.array([2], type=pa.int64()),
            pa.array([3, 7, 8], type=pa.int64()),
            pa.array([], type=pa.int64()),
            pa.array([9, 10, 11], type=pa.int64()),
        ])

        # Confirm len is num elements, not num chunks
        assert len(manual_chunked_selection) == 7
        assert len(manual_chunked_selection.chunks) == 4

        view5 = view.filter_index_values(manual_chunked_selection)
        batches = view5.select()
        table5 = pa.Table.from_batches(batches, batches.schema)

        assert table5.num_rows == 1

        # Exceptions
        with pytest.raises(ValueError):
            view.filter_index_values(pa.array([8, 8], type=pa.int64()))

        with pytest.raises(TypeError):
            view.filter_index_values("1")

        with pytest.raises(TypeError):
            view.filter_index_values(pa.array([1.0, 2.0], type=pa.float64()))

    def test_using_index_values(self) -> None:
        view = self.recording.view(index="my_index", contents="points")
        view = view.using_index_values([0, 5, 9])

        table = view.select().read_all().combine_chunks()

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 3

        expected_index = pa.chunked_array([
            pa.array(
                [0, 5, 9],
                type=pa.int64(),
            )
        ])

        assert table.column("my_index").equals(expected_index)
        assert not table.column("/points:Position3D")[0].is_valid
        assert not table.column("/points:Position3D")[1].is_valid
        assert not table.column("/points:Position3D")[2].is_valid

        table = view.fill_latest_at().select().read_all().combine_chunks()

        assert table.num_columns == 5
        assert table.num_rows == 3

        assert table.column("my_index").equals(expected_index)
        assert not table.column("/points:Position3D")[0].is_valid
        assert table.column("/points:Position3D")[1].values.equals(self.expected_pos0)
        assert table.column("/points:Position3D")[2].values.equals(self.expected_pos1)

    def test_filter_is_not_null(self) -> None:
        view = self.recording.view(index="my_index", contents="points")

        color = rr.dataframe.ComponentColumnSelector("points", rr.components.Color)

        view = view.filter_is_not_null(color)

        table = view.select().read_all()

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 1

        assert table.column("my_index")[0].equals(self.expected_index1[0])

        assert table.column("/points:Position3D")[0].values.equals(self.expected_pos1)

    def test_view_syntax(self) -> None:
        good_content_expressions = [
            {"points": rr.components.Position3D},
            {"points": [rr.components.Position3D]},
            {"points": "rerun.components.Position3D"},
            {"points/**": "rerun.components.Position3D"},
            {"points/**": "Position3D"},
            {"points/**": "position3D"},
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
