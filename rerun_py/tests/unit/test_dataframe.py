from __future__ import annotations

import pathlib
import tempfile
import uuid

import pyarrow as pa
import pytest
import rerun as rr
from rerun_bindings.rerun_bindings import Schema
from rerun_bindings.types import AnyColumn, ComponentLike, ViewContentsLike

APP_ID = "rerun_example_test_recording"
RECORDING_ID = uuid.uuid4()


def test_load_recording() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rrd = tmpdir + "/tmp.rrd"

        with rr.RecordingStream("rerun_example_test_recording") as rec:
            rec.save(rrd)
            rec.set_index("my_index", sequence=1)
            rec.log("/content/log", rr.TextLog("Hello"))

        recording = rr.dataframe.load_recording(rrd)
        assert recording is not None

        view = recording.view(index="my_index", contents="/**")
        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, text
        assert table.num_columns == 5
        assert table.num_rows == 1

        recording = rr.dataframe.load_recording(pathlib.Path(tmpdir) / "tmp.rrd")
        assert recording is not None

        view = recording.view(index="my_index", contents="/**")
        batches = view.select()
        table = pa.Table.from_batches(batches, batches.schema)

        # my_index, log_time, log_tick, text
        assert table.num_columns == 5
        assert table.num_rows == 1


class TestDataframe:
    def setup_method(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/tmp.rrd"

            with rr.RecordingStream(APP_ID, recording_id=RECORDING_ID) as rec:
                rec.save(rrd)
                rec.set_index("my_index", sequence=1)
                rec.log("/content/points", rr.Points3D([[1, 2, 3], [4, 5, 6], [7, 8, 9]], radii=[]))
                rec.set_index("my_index", sequence=7)
                rec.log("/content/points", rr.Points3D([[10, 11, 12]], colors=[[255, 0, 0]]))
                rec.log("/content/static_text", rr.TextLog("Hello"), static=True)

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

    def test_schema_recording(self) -> None:
        schema: Schema = self.recording.schema()

        # log_tick, log_time, my_index
        assert len(schema.index_columns()) == 3
        # Color, Points3DIndicator, Position3D, Radius, Text, TextIndicator
        assert len(schema.component_columns()) == 8

        # Index columns
        assert schema.index_columns()[0].name == "log_tick"
        assert schema.index_columns()[1].name == "log_time"
        assert schema.index_columns()[2].name == "my_index"

        # Default property columns
        assert schema.component_columns()[0].entity_path == "/__recording_properties"
        assert schema.component_columns()[0].component_name == "rerun.components.RecordingPropertiesIndicator"
        assert schema.component_columns()[0].is_static is True
        assert schema.component_columns()[1].entity_path == "/__recording_properties"
        assert schema.component_columns()[1].component_name == "rerun.components.Timestamp"
        assert schema.component_columns()[1].is_static is True

        # Content columns
        assert schema.component_columns()[2].entity_path == "/points"
        assert schema.component_columns()[2].component_name == "rerun.components.Points3DIndicator"
        assert schema.component_columns()[2].is_static is False
        assert schema.component_columns()[3].entity_path == "/points"
        assert schema.component_columns()[3].component_name == "rerun.components.Color"
        assert schema.component_columns()[3].is_static is False
        assert schema.component_columns()[4].entity_path == "/points"
        assert schema.component_columns()[4].component_name == "rerun.components.Position3D"
        assert schema.component_columns()[4].is_static is False
        assert schema.component_columns()[5].entity_path == "/points"
        assert schema.component_columns()[5].component_name == "rerun.components.Radius"
        assert schema.component_columns()[5].is_static is False
        assert schema.component_columns()[6].entity_path == "/static_text"
        assert schema.component_columns()[6].component_name == "rerun.components.TextLogIndicator"
        assert schema.component_columns()[6].is_static is True
        assert schema.component_columns()[7].entity_path == "/static_text"
        assert schema.component_columns()[7].component_name == "rerun.components.Text"
        assert schema.component_columns()[7].is_static is True

    def test_schema_view(self) -> None:
        schema = self.recording.view(index="my_index", contents="/content/points").schema()

        assert len(schema.index_columns()) == 3
        # Position3D, Color
        assert len(schema.component_columns()) == 2

        assert schema.index_columns()[0].name == "log_tick"
        assert schema.index_columns()[1].name == "log_time"
        assert schema.index_columns()[2].name == "my_index"
        assert schema.component_columns()[0].entity_path == "/points"
        assert schema.component_columns()[0].component_name == "rerun.components.Color"
        assert schema.component_columns()[1].entity_path == "/points"
        assert schema.component_columns()[1].component_name == "rerun.components.Position3D"

        # Force radius to be included
        schema = self.recording.view(
            index="my_index",
            contents="/content/points",
            include_semantically_empty_columns=True,
        ).schema()

        assert len(schema.index_columns()) == 3
        # Color, Position3D, Radius
        assert len(schema.component_columns()) == 3
        assert schema.component_columns()[2].component_name == "rerun.components.Radius"

    def test_full_view(self) -> None:
        view = self.recording.view(index="my_index", contents="/**")

        table = view.select().read_all()

        # my_index, log_time, log_tick, properties.started, points, colors, text
        assert table.num_columns == 7
        assert table.num_rows == 2

        table = view.select(
            columns=[col for col in view.schema() if not col.is_static],
        ).read_all()

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 2

        table = view.select_static().read_all()

        # text
        assert table.num_columns == 1
        assert table.num_rows == 1

    def test_content_filters(self) -> None:
        filter_expressions: list[ViewContentsLike] = [
            "+/** -/static_text",
            """
            +/**
            -/static_text
            """,
            {"/** -/static_text": ["Position3D", "Color"]},
        ]

        for expr in filter_expressions:
            view = self.recording.view(index="my_index", contents=expr)

            table = view.select().read_all()

            # my_index, log_time, properties.started, log_tick, points, colors
            assert table.num_columns == 6
            assert table.num_rows == 2

    def test_select_columns(self) -> None:
        view = self.recording.view(index="my_index", contents="/content/points")
        index_col_selectors: list[AnyColumn] = [rr.dataframe.IndexColumnSelector("my_index"), "my_index"]

        selectors: list[ComponentLike] = [
            rr.components.Position3D,
            "rerun.components.Position3D",
            "Position3D",
            "position3D",
        ]

        all_selectors: list[AnyColumn] = [
            *[rr.dataframe.ComponentColumnSelector("points", selector) for selector in selectors],
            "/points:rerun.components.Position3D",
            "/points:Position3D",
            "/points:position3d",
        ]

        for index_selector in index_col_selectors:
            for col_selector in all_selectors:
                batches = view.select(index_selector, col_selector)

                table = pa.Table.from_batches(batches, batches.schema)
                # points
                assert table.num_columns == 2
                assert table.num_rows == 2

                assert table.column("my_index")[0].equals(self.expected_index0[0])
                assert table.column("my_index")[1].equals(self.expected_index1[0])
                assert table.column("/points:Position3D")[0].values.equals(self.expected_pos0)
                assert table.column("/points:Position3D")[1].values.equals(self.expected_pos1)

    def test_index_values(self) -> None:
        view = self.recording.view(index="my_index", contents="/content/points")
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
        view = self.recording.view(index="my_index", contents="/content/points")
        view = view.using_index_values([0, 5, 9])

        table = view.select().read_all().combine_chunks()

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 3

        expected_index = pa.chunked_array([
            pa.array(
                [0, 5, 9],
                type=pa.int64(),
            ),
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
        view = self.recording.view(index="my_index", contents="/content/points")

        color = rr.dataframe.ComponentColumnSelector("points", rr.components.Color)

        view = view.filter_is_not_null(color)

        table = view.select().read_all()

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 1

        assert table.column("my_index")[0].equals(self.expected_index1[0])

        assert table.column("/points:Position3D")[0].values.equals(self.expected_pos1)

    def test_view_syntax(self) -> None:
        good_content_expressions: list[ViewContentsLike] = [
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

        bad_content_expressions: list[ViewContentsLike] = [
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

    def test_roundtrip_send(self) -> None:
        df = self.recording.view(index="my_index", contents="/content/**").select().read_all()

        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/tmp.rrd"

            with rr.RecordingStream("rerun_example_test_recording") as rec:
                rec.save(rrd)
                rr.dataframe.send_dataframe(df, rec=rec)

            round_trip_recording = rr.dataframe.load_recording(rrd)

        df_round_trip = round_trip_recording.view(index="my_index", contents="/content/**").select().read_all()

        print("df:")
        print(df)
        print()

        print("df_round_trip:")
        print(df_round_trip)
        print()

        assert df == df_round_trip
