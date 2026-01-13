"""Tests for view_contents_for_* helper functions."""

from __future__ import annotations

import tempfile
import uuid

import pytest
import rerun as rr


class TestViewContentsHelpers:
    """Test suite for view contents helper functions."""

    def setup_method(self) -> None:
        """Create a test recording with known data."""
        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/test_view_helpers.rrd"

            recording_id = uuid.uuid4()
            with rr.RecordingStream("test_view_helpers", recording_id=recording_id) as rec:
                rec.save(rrd)

                # Log Points3D at /world/points
                rec.set_time("frame", sequence=1)
                rec.log(
                    "/world/points",
                    rr.Points3D([[1, 2, 3], [4, 5, 6]], colors=[[255, 0, 0], [0, 255, 0]])
                )

                # Log Points3D at /world/markers
                rec.log(
                    "/world/markers",
                    rr.Points3D([[7, 8, 9]])
                )

                # Log Transform3D at /world/camera
                rec.log(
                    "/world/camera",
                    rr.Transform3D(translation=[1, 2, 3])
                )

                # Log Boxes3D at /world/boxes
                rec.log(
                    "/world/boxes",
                    rr.Boxes3D(half_sizes=[[1, 1, 1]])
                )

            self.recording = rr.dataframe.load_recording(rrd)
            self.schema = self.recording.schema()

    def test_view_contents_for_archetypes_single(self) -> None:
        """Test view_contents_for_archetypes with single archetype."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            "rerun.archetypes.Points3D"
        )

        # Should return dict format
        assert isinstance(contents, dict)
        assert "/**" in contents

        # Should contain component names for Points3D
        component_names = contents["/**"]
        assert isinstance(component_names, list)
        assert len(component_names) > 0

        # All component names should be from Points3D archetype
        all_columns = self.schema.columns_for(archetype="rerun.archetypes.Points3D")
        expected_components = [col.component for col in all_columns]
        assert set(component_names) == set(expected_components)

    def test_view_contents_for_archetypes_multiple(self) -> None:
        """Test view_contents_for_archetypes with multiple archetypes."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            ["rerun.archetypes.Points3D", "rerun.archetypes.Transform3D"]
        )

        # Should return dict format
        assert isinstance(contents, dict)
        assert "/**" in contents

        component_names = contents["/**"]

        # Should contain components from both archetypes
        points_columns = self.schema.columns_for(archetype="rerun.archetypes.Points3D")
        transform_columns = self.schema.columns_for(archetype="rerun.archetypes.Transform3D")

        expected_components = [col.component for col in points_columns] + [col.component for col in transform_columns]
        assert set(component_names) == set(expected_components)

    def test_view_contents_for_archetypes_with_entity_path(self) -> None:
        """Test view_contents_for_archetypes with entity_path filter."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            "rerun.archetypes.Points3D",
            entity_path="/world/points"
        )

        component_names = contents["/**"]

        # Should only contain components from /world/points
        expected_columns = self.schema.columns_for(
            archetype="rerun.archetypes.Points3D",
            entity_path="/world/points"
        )
        expected_components = [col.component for col in expected_columns]
        assert set(component_names) == set(expected_components)

        # Markers have the same components (Points3D:positions, etc.)
        # so we can't distinguish them by component name alone
        # This is expected behavior - the helper returns component names,
        # and the entity_path filter limits which entities are considered

    def test_view_contents_for_archetypes_empty_result(self) -> None:
        """Test view_contents_for_archetypes with non-existent archetype."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            "rerun.archetypes.NonExistent"
        )

        # Should return dict with empty list
        assert isinstance(contents, dict)
        assert contents["/**"] == []

    def test_view_contents_for_component_types_single(self) -> None:
        """Test view_contents_for_component_types with single component type."""
        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            "rerun.components.Position3D"
        )

        # Should return dict format
        assert isinstance(contents, dict)
        assert "/**" in contents

        component_names = contents["/**"]
        assert isinstance(component_names, list)
        assert len(component_names) > 0

        # All component names should be from Position3D component type
        all_columns = self.schema.columns_for(component_type="rerun.components.Position3D")
        expected_components = [col.component for col in all_columns]
        assert set(component_names) == set(expected_components)

    def test_view_contents_for_component_types_multiple(self) -> None:
        """Test view_contents_for_component_types with multiple component types."""
        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            ["rerun.components.Position3D", "rerun.components.Color"]
        )

        component_names = contents["/**"]

        # Should contain components from both component types
        position_columns = self.schema.columns_for(component_type="rerun.components.Position3D")
        color_columns = self.schema.columns_for(component_type="rerun.components.Color")

        expected_components = [col.component for col in position_columns] + [col.component for col in color_columns]
        assert set(component_names) == set(expected_components)

    def test_view_contents_for_component_types_with_entity_path(self) -> None:
        """Test view_contents_for_component_types with entity_path filter."""
        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            "rerun.components.Position3D",
            entity_path="/world/points"
        )

        component_names = contents["/**"]

        # Should only contain components from /world/points
        expected_columns = self.schema.columns_for(
            component_type="rerun.components.Position3D",
            entity_path="/world/points"
        )
        expected_components = [col.component for col in expected_columns]
        assert set(component_names) == set(expected_components)

    def test_view_contents_for_component_types_empty_result(self) -> None:
        """Test view_contents_for_component_types with non-existent component type."""
        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            "rerun.components.NonExistent"
        )

        # Should return dict with empty list
        assert isinstance(contents, dict)
        assert contents["/**"] == []

    def test_integration_create_view_with_archetype_filter(self) -> None:
        """Test creating a RecordingView using view_contents_for_archetypes."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            "rerun.archetypes.Points3D"
        )

        # Should be able to create a view with these contents
        view = self.recording.view(index="frame", contents=contents)

        # View should only contain Points3D columns
        view_schema = view.schema()
        view_component_columns = view_schema.component_columns()

        for col in view_component_columns:
            assert col.archetype == "rerun.archetypes.Points3D"

    def test_integration_create_view_with_component_type_filter(self) -> None:
        """Test creating a RecordingView using view_contents_for_component_types."""
        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            "rerun.components.Position3D"
        )

        # Should be able to create a view with these contents
        view = self.recording.view(index="frame", contents=contents)

        # View should only contain Position3D columns
        view_schema = view.schema()
        view_component_columns = view_schema.component_columns()

        for col in view_component_columns:
            assert col.component_type == "rerun.components.Position3D"

    def test_integration_create_view_multiple_archetypes(self) -> None:
        """Test creating view with multiple archetypes."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            ["rerun.archetypes.Points3D", "rerun.archetypes.Boxes3D"]
        )

        view = self.recording.view(index="frame", contents=contents)
        view_schema = view.schema()
        archetypes = view_schema.archetypes()

        # Should contain both archetypes
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Boxes3D" in archetypes
        # Should NOT contain Transform3D
        assert "rerun.archetypes.Transform3D" not in archetypes


class TestTypeConversion:
    """Test suite for type conversion in view contents helpers."""

    def setup_method(self) -> None:
        """Create a test recording with known data."""
        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/test_type_conversion.rrd"

            recording_id = uuid.uuid4()
            with rr.RecordingStream("test_type_conversion", recording_id=recording_id) as rec:
                rec.save(rrd)

                # Log Points3D
                rec.set_time("frame", sequence=1)
                rec.log(
                    "/world/points",
                    rr.Points3D([[1, 2, 3]], colors=[[255, 0, 0]])
                )

            self.recording = rr.dataframe.load_recording(rrd)
            self.schema = self.recording.schema()

    def test_archetype_type_single(self) -> None:
        """Test view_contents_for_archetypes with single archetype type."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            rr.Points3D
        )

        # Should work the same as string version
        contents_str = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            "rerun.archetypes.Points3D"
        )

        assert contents == contents_str

    def test_archetype_type_multiple(self) -> None:
        """Test view_contents_for_archetypes with multiple archetype types."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            [rr.Points3D, rr.Transform3D]
        )

        contents_str = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            ["rerun.archetypes.Points3D", "rerun.archetypes.Transform3D"]
        )

        assert contents == contents_str

    def test_archetype_type_mixed(self) -> None:
        """Test view_contents_for_archetypes with mixed types and strings."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            [rr.Points3D, "rerun.archetypes.Transform3D"]
        )

        contents_all_str = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            ["rerun.archetypes.Points3D", "rerun.archetypes.Transform3D"]
        )

        assert contents == contents_all_str

    def test_archetype_type_with_entity_path(self) -> None:
        """Test archetype type with entity_path parameter."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            rr.Points3D,
            entity_path="/world/points"
        )

        contents_str = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            "rerun.archetypes.Points3D",
            entity_path="/world/points"
        )

        assert contents == contents_str

    def test_component_type_single(self) -> None:
        """Test view_contents_for_component_types with single component type."""
        from rerun import components

        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            components.Position3D
        )

        contents_str = rr.dataframe.view_contents_for_component_types(
            self.schema,
            "rerun.components.Position3D"
        )

        assert contents == contents_str

    def test_component_type_multiple(self) -> None:
        """Test view_contents_for_component_types with multiple component types."""
        from rerun import components

        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            [components.Position3D, components.Color]
        )

        contents_str = rr.dataframe.view_contents_for_component_types(
            self.schema,
            ["rerun.components.Position3D", "rerun.components.Color"]
        )

        assert contents == contents_str

    def test_component_type_mixed(self) -> None:
        """Test view_contents_for_component_types with mixed types and strings."""
        from rerun import components

        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            [components.Position3D, "rerun.components.Color"]
        )

        contents_all_str = rr.dataframe.view_contents_for_component_types(
            self.schema,
            ["rerun.components.Position3D", "rerun.components.Color"]
        )

        assert contents == contents_all_str

    def test_component_type_with_entity_path(self) -> None:
        """Test component type with entity_path parameter."""
        from rerun import components

        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            components.Position3D,
            entity_path="/world/points"
        )

        contents_str = rr.dataframe.view_contents_for_component_types(
            self.schema,
            "rerun.components.Position3D",
            entity_path="/world/points"
        )

        assert contents == contents_str

    def test_integration_archetype_type_creates_view(self) -> None:
        """Test that views can be created using archetype types."""
        contents = rr.dataframe.view_contents_for_archetypes(
            self.schema,
            rr.Points3D
        )

        # Should successfully create view
        view = self.recording.view(index="frame", contents=contents)
        view_schema = view.schema()

        # View should contain Points3D
        archetypes = view_schema.archetypes()
        assert "rerun.archetypes.Points3D" in archetypes

    def test_integration_component_type_creates_view(self) -> None:
        """Test that views can be created using component types."""
        from rerun import components

        contents = rr.dataframe.view_contents_for_component_types(
            self.schema,
            components.Position3D
        )

        # Should successfully create view
        view = self.recording.view(index="frame", contents=contents)
        view_schema = view.schema()

        # View should contain Position3D
        component_types = view_schema.component_types()
        assert "rerun.components.Position3D" in component_types
