from __future__ import annotations

import pathlib
import tempfile
import uuid

import pyarrow as pa
import pytest
import rerun as rr
from rerun_bindings.rerun_bindings import Schema
from rerun_bindings.types import AnyColumn, ViewContentsLike

APP_ID = "rerun_example_test_recording"


def test_load_recording() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rrd = tmpdir + "/tmp.rrd"

        with rr.RecordingStream("rerun_example_test_recording", recording_id=uuid.uuid4()) as rec:
            rec.save(rrd)
            rec.set_time("my_index", sequence=1)
            rec.log("log", rr.TextLog("Hello"))

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
        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/tmp.rrd"

            self.expected_recording_id = uuid.uuid4()
            with rr.RecordingStream(APP_ID, recording_id=self.expected_recording_id) as rec:
                rec.save(rrd)
                rec.set_time("my_index", sequence=1)
                rec.log("points", rr.Points3D([[1, 2, 3], [4, 5, 6], [7, 8, 9]], radii=[]))
                rec.set_time("my_index", sequence=7)
                rec.log("points", rr.Points3D([[10, 11, 12]], colors=[[255, 0, 0]]))
                rec.log("static_text", rr.TextLog("Hello"), static=True)

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
        assert self.recording.recording_id() == str(self.expected_recording_id)

    def test_schema_recording(self) -> None:
        schema: Schema = self.recording.schema()

        # log_tick, log_time, my_index
        assert len(schema.index_columns()) == 3
        # RecordingPropertiesIndicator, Timestamp, Color, Points3DIndicator, Position3D, Radius, Text, TextIndicator
        assert len(schema.component_columns()) == 8

        # Index columns
        assert schema.index_columns()[0].name == "log_tick"
        assert schema.index_columns()[1].name == "log_time"
        assert schema.index_columns()[2].name == "my_index"

        col = 0

        # Content columns
        assert schema.component_columns()[col].entity_path == "/points"
        assert schema.component_columns()[col].archetype is None
        assert schema.component_columns()[col].component == "rerun.components.Points3DIndicator"
        assert schema.component_columns()[col].component_type is None
        assert schema.component_columns()[col].is_static is False
        col += 1

        assert schema.component_columns()[col].entity_path == "/points"
        assert schema.component_columns()[col].archetype == "rerun.archetypes.Points3D"
        assert schema.component_columns()[col].component == "Points3D:colors"
        assert schema.component_columns()[col].component_type == "rerun.components.Color"
        assert schema.component_columns()[col].is_static is False
        col += 1

        assert schema.component_columns()[col].entity_path == "/points"
        assert schema.component_columns()[col].archetype == "rerun.archetypes.Points3D"
        assert schema.component_columns()[col].component == "Points3D:positions"
        assert schema.component_columns()[col].component_type == "rerun.components.Position3D"
        assert schema.component_columns()[col].is_static is False
        col += 1

        assert schema.component_columns()[col].entity_path == "/points"
        assert schema.component_columns()[col].archetype == "rerun.archetypes.Points3D"
        assert schema.component_columns()[col].component == "Points3D:radii"
        assert schema.component_columns()[col].component_type == "rerun.components.Radius"
        assert schema.component_columns()[col].is_static is False
        col += 1

        assert schema.component_columns()[col].entity_path == "/static_text"
        assert schema.component_columns()[col].archetype is None
        assert schema.component_columns()[col].component == "rerun.components.TextLogIndicator"
        assert schema.component_columns()[col].component_type is None
        assert schema.component_columns()[col].is_static is True
        col += 1

        assert schema.component_columns()[col].entity_path == "/static_text"
        assert schema.component_columns()[col].archetype == "rerun.archetypes.TextLog"
        assert schema.component_columns()[col].component == "TextLog:text"
        assert schema.component_columns()[col].component_type == "rerun.components.Text"
        assert schema.component_columns()[col].is_static is True
        col += 1

        # Default property columns
        assert schema.component_columns()[col].entity_path == "/__properties/recording"
        assert schema.component_columns()[col].archetype is None
        assert schema.component_columns()[col].component == "rerun.components.RecordingPropertiesIndicator"
        assert schema.component_columns()[col].component_type is None
        assert schema.component_columns()[col].is_static is True
        col += 1

        assert schema.component_columns()[col].entity_path == "/__properties/recording"
        assert schema.component_columns()[col].archetype == "rerun.archetypes.RecordingProperties"
        assert schema.component_columns()[col].component == "RecordingProperties:start_time"
        assert schema.component_columns()[col].component_type == "rerun.components.Timestamp"
        assert schema.component_columns()[col].is_static is True

    def test_schema_view(self) -> None:
        schema = self.recording.view(index="my_index", contents="points").schema()

        assert len(schema.index_columns()) == 3
        # Position3D, Color
        assert len(schema.component_columns()) == 2

        assert schema.index_columns()[0].name == "log_tick"
        assert schema.index_columns()[1].name == "log_time"
        assert schema.index_columns()[2].name == "my_index"
        assert schema.component_columns()[0].entity_path == "/points"
        assert schema.component_columns()[0].archetype == "rerun.archetypes.Points3D"
        assert schema.component_columns()[0].component == "Points3D:colors"
        assert schema.component_columns()[0].component_type == "rerun.components.Color"
        assert schema.component_columns()[1].entity_path == "/points"
        assert schema.component_columns()[1].archetype == "rerun.archetypes.Points3D"
        assert schema.component_columns()[1].component == "Points3D:positions"
        assert schema.component_columns()[1].component_type == "rerun.components.Position3D"

        # Force radius to be included
        schema = self.recording.view(
            index="my_index",
            contents="points",
            include_semantically_empty_columns=True,
        ).schema()

        assert len(schema.index_columns()) == 3
        # Color, Position3D, Radius
        assert len(schema.component_columns()) == 3
        assert schema.component_columns()[2].component_type == "rerun.components.Radius"

    def test_full_view(self) -> None:
        view = self.recording.view(index="my_index", contents="/**")

        table = view.select().read_all()

        # my_index, log_time, log_tick, points, colors, text
        assert table.num_columns == 6
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
            {"/** -/static_text": ["Points3D:positions", "Points3D:colors"]},
        ]

        for expr in filter_expressions:
            view = self.recording.view(index="my_index", contents=expr)

            table = view.select().read_all()

            # my_index, log_time, log_tick, points, colors
            assert table.num_columns == 5
            assert table.num_rows == 2

    def test_select_columns(self) -> None:
        view = self.recording.view(index="my_index", contents="points")
        index_col_selectors: list[AnyColumn] = [rr.dataframe.IndexColumnSelector("my_index"), "my_index"]

        selectors: list[str] = ["Points3D:positions"]

        all_selectors: list[AnyColumn] = [
            *[rr.dataframe.ComponentColumnSelector("points", selector) for selector in selectors],
            "/points:Points3D:positions",
        ]

        for index_selector in index_col_selectors:
            for col_selector in all_selectors:
                batches = view.select(index_selector, col_selector)

                table = pa.Table.from_batches(batches, batches.schema)
                # points
                assert table.num_columns == 2
                assert table.num_rows == 2

                print("\n\n")
                print(f"index_selector: {index_selector}")
                print(f"col_selector: {col_selector}")
                print(f"table.schema: {table.schema}")
                assert table.column("my_index")[0].equals(self.expected_index0[0]), f"col_selector: {col_selector}"
                assert table.column("my_index")[1].equals(self.expected_index1[0]), f"col_selector: {col_selector}"
                assert table.column("/points:Points3D:positions")[0].values.equals(self.expected_pos0), (
                    f"col_selector: {col_selector}"
                )
                assert table.column("/points:Points3D:positions")[1].values.equals(self.expected_pos1), (
                    f"col_selector: {col_selector}"
                )

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
            ),
        ])

        assert table.column("my_index").equals(expected_index)
        assert not table.column("/points:Points3D:positions")[0].is_valid
        assert not table.column("/points:Points3D:positions")[1].is_valid
        assert not table.column("/points:Points3D:positions")[2].is_valid

        table = view.fill_latest_at().select().read_all().combine_chunks()

        assert table.num_columns == 5
        assert table.num_rows == 3

        assert table.column("my_index").equals(expected_index)
        assert not table.column("/points:Points3D:positions")[0].is_valid
        assert table.column("/points:Points3D:positions")[1].values.equals(self.expected_pos0)
        assert table.column("/points:Points3D:positions")[2].values.equals(self.expected_pos1)

    def test_filter_is_not_null(self) -> None:
        view = self.recording.view(index="my_index", contents="points")

        color = rr.dataframe.ComponentColumnSelector("points", "Points3D:colors")

        view = view.filter_is_not_null(color)

        table = view.select().read_all()

        # my_index, log_time, log_tick, points, colors
        assert table.num_columns == 5
        assert table.num_rows == 1

        assert table.column("my_index")[0].equals(self.expected_index1[0])

        assert table.column("/points:Points3D:positions")[0].values.equals(self.expected_pos1)

    def test_view_syntax(self) -> None:
        good_content_expressions: list[ViewContentsLike] = [
            {"points": "Points3D:positions"},
            {"points/**": "Points3D:positions"},
        ]

        for expr in good_content_expressions:
            view = self.recording.view(index="my_index", contents=expr)
            batches = view.select()
            table = pa.Table.from_batches(batches, batches.schema)

            # my_index, log_time, log_tick, points
            assert table.num_columns == 4
            assert table.num_rows == 2

        bad_content_expressions: list[ViewContentsLike] = [
            # We don't support selecting by components anymore.
            {"points": "rerun.components.Position3D"},
            {"points/**": "rerun.components.Position3D"},
        ]

        for expr in bad_content_expressions:
            view = self.recording.view(index="my_index", contents=expr)
            batches = view.select()

            # my_index, log_time, log_tick
            table = pa.Table.from_batches(batches, batches.schema)
            assert table.num_columns == 3
            assert table.num_rows == 0

    def test_roundtrip_send(self) -> None:
        df = self.recording.view(index="my_index", contents="/**").select().read_all()

        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/tmp.rrd"

            with rr.RecordingStream("rerun_example_test_recording", recording_id=uuid.uuid4()) as rec:
                rec.save(rrd)
                rr.dataframe.send_dataframe(df, rec=rec)

            round_trip_recording = rr.dataframe.load_recording(rrd)

        df_round_trip = round_trip_recording.view(index="my_index", contents="/**").select().read_all()

        print("df:")
        print(df)
        print()

        print("df_round_trip:")
        print(df_round_trip)
        print()

        assert df == df_round_trip


@pytest.fixture
def any_value_static_recording(tmp_path: pathlib.Path) -> rr.dataframe.Recording:
    """A recording with just a static AnyValues archetype."""

    rrd_path = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd_path)
        rec.log(
            "/test",
            # Note: defensive parameter names to avoid collision with `AnyValues` tracking of per-component types.
            rr.AnyValues(test_dataframe_yak="yuk", test_dataframe_foo="bar", test_dataframe_baz=42),
            static=True,
        )

    # Use this to exfiltrate the RRD file for debugging purposes:
    if False:
        import shutil

        shutil.copy(rrd_path, "/tmp/exfiltrated.rrd")

    recording = rr.dataframe.load_recording(rrd_path)
    assert recording is not None

    return recording


def test_dataframe_static(any_value_static_recording: rr.dataframe.Recording) -> None:
    view = any_value_static_recording.view(index=None, contents="/**")

    table = view.select().read_all()

    assert table.column(0).to_pylist()[0] is not None
    assert table.column(1).to_pylist()[0] is not None
    assert table.column(1).to_pylist()[0] is not None


def test_dataframe_index_no_default(any_value_static_recording: rr.dataframe.Recording) -> None:
    """We specifically want index to not default None. This must be explicitly set to indicate a static query."""
    with pytest.raises(TypeError, match="missing 1 required keyword argument"):
        any_value_static_recording.view(contents="/**")  # type: ignore[call-arg]


@pytest.fixture
def mixed_static_recording(tmp_path: pathlib.Path) -> rr.dataframe.Recording:
    """A recording with a mix of regular and AnyValues static archetypes."""
    rrd_path = tmp_path / "tmp.rrd"

    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(rrd_path)
        rec.log(
            "/test",
            # Note: defensive parameter names to avoid collision with `AnyValues` tracking of per-component types.
            rr.AnyValues(test_dataframe_yak="yuk", test_dataframe_foo="bar", test_dataframe_baz=42),
            static=True,
        )
        rec.log("/test2", rr.Points3D([1, 2, 3], radii=5), static=True)

    recording = rr.dataframe.load_recording(rrd_path)
    assert recording is not None

    return recording


# TODO(#10335): remove when `select_static` is removed.
def test_dataframe_static_new_vs_deprecated(mixed_static_recording: rr.dataframe.Recording) -> None:
    """Assert that the new `index=None` method yields the same results as the deprecated `select_static` method."""
    view1 = mixed_static_recording.view(
        index=None,
        contents="/**",
    )
    table1 = view1.select().read_all()

    view2 = mixed_static_recording.view(
        index="log_time",
        contents="/**",
    )
    table2 = view2.select_static().read_all()

    assert table1 == table2
