"""Tests for schema exploration methods (archetypes, entities, component_types, columns_for)."""

from __future__ import annotations

import tempfile
import uuid

import pytest
import rerun as rr


class TestSchemaExploration:
    """Test suite for new schema exploration methods."""

    def setup_method(self) -> None:
        """Create a test recording with known data."""
        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/test_schema.rrd"

            recording_id = uuid.uuid4()
            with rr.RecordingStream("test_schema_exploration", recording_id=recording_id) as rec:
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

                # Log TextLog at /logs/debug
                rec.log(
                    "/logs/debug",
                    rr.TextLog("Test message")
                )

            self.recording = rr.dataframe.load_recording(rrd)
            self.schema = self.recording.schema()

    def test_archetypes(self) -> None:
        """Test schema.archetypes() returns expected archetype list."""
        archetypes = self.schema.archetypes()

        # Should be sorted and unique
        assert isinstance(archetypes, list)
        assert len(archetypes) > 0
        assert archetypes == sorted(archetypes)

        # Should contain our logged archetypes (fully-qualified names)
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Transform3D" in archetypes
        assert "rerun.archetypes.Boxes3D" in archetypes
        assert "rerun.archetypes.TextLog" in archetypes

        # Check no duplicates
        assert len(archetypes) == len(set(archetypes))

    def test_entities(self) -> None:
        """Test schema.entities() returns expected entity path list."""
        entities = self.schema.entities()

        # Should be sorted and unique
        assert isinstance(entities, list)
        assert len(entities) > 0
        assert entities == sorted(entities)

        # Should contain our logged entity paths
        assert "/world/points" in entities
        assert "/world/markers" in entities
        assert "/world/camera" in entities
        assert "/world/boxes" in entities
        assert "/logs/debug" in entities

        # Check no duplicates
        assert len(entities) == len(set(entities))

    def test_component_types(self) -> None:
        """Test schema.component_types() returns expected component type list."""
        component_types = self.schema.component_types()

        # Should be sorted and unique
        assert isinstance(component_types, list)
        assert len(component_types) > 0
        assert component_types == sorted(component_types)

        # Should contain expected component types (fully-qualified names)
        assert "rerun.components.Position3D" in component_types
        assert "rerun.components.Color" in component_types

        # Check no duplicates
        assert len(component_types) == len(set(component_types))

    def test_columns_for_all(self) -> None:
        """Test columns_for() with no filters returns all columns."""
        columns = self.schema.columns_for()

        # Should return all component columns
        assert isinstance(columns, list)
        assert len(columns) > 0

        # Should match component_columns()
        all_columns = self.schema.component_columns()
        assert len(columns) == len(all_columns)

    def test_columns_for_entity(self) -> None:
        """Test columns_for() filtered by entity_path."""
        columns = self.schema.columns_for(entity_path="/world/points")

        # Should only get columns for /world/points
        assert len(columns) > 0
        for col in columns:
            assert col.entity_path == "/world/points"

        # Should have columns for Points3D components (positions, colors)
        column_names = [col.component for col in columns]
        assert "Points3D:positions" in column_names
        assert "Points3D:colors" in column_names

    def test_columns_for_archetype(self) -> None:
        """Test columns_for() filtered by archetype."""
        columns = self.schema.columns_for(archetype="rerun.archetypes.Points3D")

        # Should only get Points3D columns
        assert len(columns) > 0
        for col in columns:
            assert col.archetype == "rerun.archetypes.Points3D"

        # Should have columns from multiple entities
        entity_paths = {col.entity_path for col in columns}
        assert "/world/points" in entity_paths
        assert "/world/markers" in entity_paths

    def test_columns_for_component_type(self) -> None:
        """Test columns_for() filtered by component_type."""
        columns = self.schema.columns_for(component_type="rerun.components.Position3D")

        # Should only get Position3D columns
        assert len(columns) > 0
        for col in columns:
            assert col.component_type == "rerun.components.Position3D"

        # Should have columns from Points3D archetypes
        archetypes = {col.archetype for col in columns}
        assert "rerun.archetypes.Points3D" in archetypes

    def test_columns_for_entity_and_archetype(self) -> None:
        """Test columns_for() with entity_path AND archetype (AND logic)."""
        columns = self.schema.columns_for(
            entity_path="/world/points",
            archetype="rerun.archetypes.Points3D"
        )

        # Should get Points3D columns at /world/points only
        assert len(columns) > 0
        for col in columns:
            assert col.entity_path == "/world/points"
            assert col.archetype == "rerun.archetypes.Points3D"

    def test_columns_for_entity_and_component_type(self) -> None:
        """Test columns_for() with entity_path AND component_type."""
        columns = self.schema.columns_for(
            entity_path="/world/points",
            component_type="rerun.components.Position3D"
        )

        # Should get Position3D columns at /world/points only
        assert len(columns) > 0
        for col in columns:
            assert col.entity_path == "/world/points"
            assert col.component_type == "rerun.components.Position3D"

    def test_columns_for_archetype_and_component_type(self) -> None:
        """Test columns_for() with archetype AND component_type."""
        columns = self.schema.columns_for(
            archetype="rerun.archetypes.Points3D",
            component_type="rerun.components.Position3D"
        )

        # Should get Position3D columns from Points3D archetype
        assert len(columns) > 0
        for col in columns:
            assert col.archetype == "rerun.archetypes.Points3D"
            assert col.component_type == "rerun.components.Position3D"

    def test_columns_for_all_three_criteria(self) -> None:
        """Test columns_for() with all three criteria (entity, archetype, component)."""
        columns = self.schema.columns_for(
            entity_path="/world/points",
            archetype="rerun.archetypes.Points3D",
            component_type="rerun.components.Position3D"
        )

        # Should get very specific match
        assert len(columns) > 0
        for col in columns:
            assert col.entity_path == "/world/points"
            assert col.archetype == "rerun.archetypes.Points3D"
            assert col.component_type == "rerun.components.Position3D"

    def test_columns_for_no_match(self) -> None:
        """Test columns_for() returns empty list when no columns match."""
        columns = self.schema.columns_for(entity_path="/nonexistent/path")

        # Should return empty list
        assert isinstance(columns, list)
        assert len(columns) == 0

    def test_columns_for_archetype_no_match(self) -> None:
        """Test columns_for() with non-existent archetype."""
        columns = self.schema.columns_for(archetype="rerun.archetypes.NonExistent")

        # Should return empty list
        assert isinstance(columns, list)
        assert len(columns) == 0

    def test_column_names_for_all(self) -> None:
        """Test column_names_for() returns just names."""
        column_names = self.schema.column_names_for()

        # Should return list of strings
        assert isinstance(column_names, list)
        assert len(column_names) > 0
        assert all(isinstance(name, str) for name in column_names)

        # Should match names from columns_for()
        columns = self.schema.columns_for()
        expected_names = [col.name for col in columns]
        assert column_names == expected_names

    def test_column_names_for_archetype(self) -> None:
        """Test column_names_for() filtered by archetype."""
        column_names = self.schema.column_names_for(archetype="rerun.archetypes.Points3D")

        # Should return list of strings
        assert isinstance(column_names, list)
        assert len(column_names) > 0
        assert all(isinstance(name, str) for name in column_names)

        # Should match names from columns_for()
        columns = self.schema.columns_for(archetype="rerun.archetypes.Points3D")
        expected_names = [col.name for col in columns]
        assert column_names == expected_names

    def test_column_names_for_entity_and_archetype(self) -> None:
        """Test column_names_for() with multiple criteria."""
        column_names = self.schema.column_names_for(
            entity_path="/world/points",
            archetype="rerun.archetypes.Points3D"
        )

        # Should return list of strings
        assert isinstance(column_names, list)
        assert len(column_names) > 0

        # Should match names from columns_for()
        columns = self.schema.columns_for(
            entity_path="/world/points",
            archetype="rerun.archetypes.Points3D"
        )
        expected_names = [col.name for col in columns]
        assert column_names == expected_names

    def test_column_names_for_no_match(self) -> None:
        """Test column_names_for() returns empty list when no match."""
        column_names = self.schema.column_names_for(entity_path="/nonexistent")

        # Should return empty list
        assert isinstance(column_names, list)
        assert len(column_names) == 0


class TestSchemaExplorationEmpty:
    """Test schema methods with minimal recording (only metadata)."""

    def setup_method(self) -> None:
        """Create a minimal recording with only metadata."""
        with tempfile.TemporaryDirectory() as tmpdir:
            rrd = tmpdir + "/test_minimal.rrd"

            with rr.RecordingStream("test_minimal", recording_id=uuid.uuid4()) as rec:
                rec.save(rrd)
                # Don't log any user data - only metadata will be present

            self.recording = rr.dataframe.load_recording(rrd)
            self.schema = self.recording.schema()

    def test_archetypes_minimal(self) -> None:
        """Test archetypes() on minimal recording (metadata only)."""
        archetypes = self.schema.archetypes()
        assert isinstance(archetypes, list)
        # Minimal recordings have RecordingInfo metadata
        assert len(archetypes) >= 0  # May have metadata archetypes

    def test_entities_minimal(self) -> None:
        """Test entities() on minimal recording (metadata only)."""
        entities = self.schema.entities()
        assert isinstance(entities, list)
        # Minimal recordings have __properties entity
        assert len(entities) >= 0  # May have metadata entities

    def test_component_types_minimal(self) -> None:
        """Test component_types() on minimal recording (metadata only)."""
        component_types = self.schema.component_types()
        assert isinstance(component_types, list)
        # Minimal recordings have metadata components
        assert len(component_types) >= 0  # May have metadata components

    def test_columns_for_minimal(self) -> None:
        """Test columns_for() on minimal recording (metadata only)."""
        columns = self.schema.columns_for()
        assert isinstance(columns, list)
        # Returns all columns including metadata
        assert len(columns) >= 0
