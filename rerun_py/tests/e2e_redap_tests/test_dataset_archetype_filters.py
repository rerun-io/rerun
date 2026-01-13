"""Tests for DatasetEntry.filter_archetypes() and filter_component_types() methods."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr

if TYPE_CHECKING:
    from rerun.catalog import CatalogClient, DatasetEntry

    from e2e_redap_tests.conftest import EntryFactory


@pytest.fixture
def test_dataset_with_archetypes(
    entry_factory: EntryFactory, catalog_client: CatalogClient
) -> DatasetEntry:
    """Create a test dataset with various archetypes."""
    import tempfile
    import uuid
    from pathlib import Path

    # Create a temporary recording with known archetypes
    with tempfile.TemporaryDirectory() as tmpdir:
        rrd = f"{tmpdir}/test_archetypes.rrd"

        recording_id = uuid.uuid4()
        with rr.RecordingStream("test_archetypes", recording_id=recording_id) as rec:
            rec.save(rrd)

            # Log Points3D at /world/points
            rec.set_time("frame", sequence=1)
            rec.log(
                "/world/points",
                rr.Points3D([[1, 2, 3], [4, 5, 6]], colors=[[255, 0, 0], [0, 255, 0]]),
            )

            # Log more Points3D at /world/markers
            rec.log("/world/markers", rr.Points3D([[7, 8, 9]]))

            # Log Transform3D at /world/camera
            rec.log("/world/camera", rr.Transform3D(translation=[1, 2, 3]))

            # Log Boxes3D at /world/boxes
            rec.log("/world/boxes", rr.Boxes3D(half_sizes=[[1, 1, 1]]))

            # Log second frame with more data
            rec.set_time("frame", sequence=2)
            rec.log("/world/points", rr.Points3D([[10, 11, 12]]))
            rec.log("/world/camera", rr.Transform3D(translation=[2, 3, 4]))

        # Create dataset and register the recording
        dataset = entry_factory.create_dataset("test_archetypes")
        # Convert tmpdir to a file:// URL
        tmpdir_url = Path(tmpdir).as_uri()
        handle = dataset.register_prefix(tmpdir_url)
        handle.wait(timeout_secs=30)

        return dataset


class TestFilterArchetypes:
    """Test suite for filter_archetypes() method."""

    def test_filter_archetypes_single_string(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by a single archetype using string."""
        view = test_dataset_with_archetypes.filter_archetypes(
            "rerun.archetypes.Points3D"
        )
        schema = view.schema()

        # View should only contain Points3D archetype
        archetypes = schema.archetypes()
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Transform3D" not in archetypes
        assert "rerun.archetypes.Boxes3D" not in archetypes

    def test_filter_archetypes_single_type(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by a single archetype using type."""
        view = test_dataset_with_archetypes.filter_archetypes(rr.Points3D)
        schema = view.schema()

        # View should only contain Points3D archetype
        archetypes = schema.archetypes()
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Transform3D" not in archetypes
        assert "rerun.archetypes.Boxes3D" not in archetypes

    def test_filter_archetypes_multiple_strings(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by multiple archetypes using strings."""
        view = test_dataset_with_archetypes.filter_archetypes(
            ["rerun.archetypes.Points3D", "rerun.archetypes.Transform3D"]
        )
        schema = view.schema()

        # View should contain both Points3D and Transform3D
        archetypes = schema.archetypes()
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Transform3D" in archetypes
        assert "rerun.archetypes.Boxes3D" not in archetypes

    def test_filter_archetypes_multiple_types(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by multiple archetypes using types."""
        view = test_dataset_with_archetypes.filter_archetypes(
            [rr.Points3D, rr.Transform3D]
        )
        schema = view.schema()

        # View should contain both Points3D and Transform3D
        archetypes = schema.archetypes()
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Transform3D" in archetypes
        assert "rerun.archetypes.Boxes3D" not in archetypes

    def test_filter_archetypes_mixed_types_and_strings(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by mixing types and strings."""
        view = test_dataset_with_archetypes.filter_archetypes(
            [rr.Points3D, "rerun.archetypes.Transform3D"]
        )
        schema = view.schema()

        # View should contain both Points3D and Transform3D
        archetypes = schema.archetypes()
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Transform3D" in archetypes
        assert "rerun.archetypes.Boxes3D" not in archetypes

    def test_filter_archetypes_nonexistent(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by non-existent archetype returns empty view."""
        view = test_dataset_with_archetypes.filter_archetypes(
            "rerun.archetypes.NonExistent"
        )
        schema = view.schema()

        # View should have no component columns
        assert len(schema.component_columns()) == 0

    def test_filter_archetypes_invalid_string_format(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test that short archetype names raise ValueError."""
        with pytest.raises(ValueError, match="must be fully-qualified"):
            test_dataset_with_archetypes.filter_archetypes("Points3D")

    def test_filter_archetypes_chain_with_segments(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test chaining filter_archetypes with filter_segments."""
        # Get first segment
        all_segments = sorted(test_dataset_with_archetypes.segment_ids())
        assert len(all_segments) >= 1

        first_segment = all_segments[0]

        # Chain filters
        view = (
            test_dataset_with_archetypes.filter_segments([first_segment])
            .filter_archetypes(rr.Points3D)
        )

        # Should have filtered by both segment and archetype
        assert view.segment_ids() == [first_segment]
        schema = view.schema()
        archetypes = schema.archetypes()
        assert "rerun.archetypes.Points3D" in archetypes
        assert "rerun.archetypes.Transform3D" not in archetypes

    def test_filter_archetypes_chain_with_contents(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test chaining filter_archetypes with filter_contents."""
        # Filter by entity path and archetype
        view = test_dataset_with_archetypes.filter_contents(
            ["/world/points"]
        ).filter_archetypes(rr.Points3D)

        schema = view.schema()

        # Should only have Points3D at /world/points
        component_columns = schema.component_columns()
        assert len(component_columns) > 0
        for col in component_columns:
            assert col.archetype == "rerun.archetypes.Points3D"
            assert col.entity_path == "/world/points"


class TestFilterComponentTypes:
    """Test suite for filter_component_types() method."""

    def test_filter_component_types_single_string(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by a single component type using string."""
        from rerun import components

        view = test_dataset_with_archetypes.filter_component_types(
            "rerun.components.Position3D"
        )
        schema = view.schema()

        # View should only contain Position3D component type
        component_types = schema.component_types()
        assert "rerun.components.Position3D" in component_types

        # All component columns should be Position3D
        component_columns = schema.component_columns()
        position_columns = [
            col
            for col in component_columns
            if col.component_type == "rerun.components.Position3D"
        ]
        assert len(position_columns) > 0

    def test_filter_component_types_single_type(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by a single component type using type."""
        from rerun import components

        view = test_dataset_with_archetypes.filter_component_types(
            components.Position3D
        )
        schema = view.schema()

        # View should only contain Position3D component type
        component_types = schema.component_types()
        assert "rerun.components.Position3D" in component_types

    def test_filter_component_types_multiple_strings(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by multiple component types using strings."""
        view = test_dataset_with_archetypes.filter_component_types(
            ["rerun.components.Position3D", "rerun.components.Color"]
        )
        schema = view.schema()

        # View should contain both Position3D and Color
        component_types = schema.component_types()
        assert "rerun.components.Position3D" in component_types
        assert "rerun.components.Color" in component_types

    def test_filter_component_types_multiple_types(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by multiple component types using types."""
        from rerun import components

        view = test_dataset_with_archetypes.filter_component_types(
            [components.Position3D, components.Color]
        )
        schema = view.schema()

        # View should contain both Position3D and Color
        component_types = schema.component_types()
        assert "rerun.components.Position3D" in component_types
        assert "rerun.components.Color" in component_types

    def test_filter_component_types_mixed_types_and_strings(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by mixing types and strings."""
        from rerun import components

        view = test_dataset_with_archetypes.filter_component_types(
            [components.Position3D, "rerun.components.Color"]
        )
        schema = view.schema()

        # View should contain both Position3D and Color
        component_types = schema.component_types()
        assert "rerun.components.Position3D" in component_types
        assert "rerun.components.Color" in component_types

    def test_filter_component_types_nonexistent(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test filtering by non-existent component type returns empty view."""
        view = test_dataset_with_archetypes.filter_component_types(
            "rerun.components.NonExistent"
        )
        schema = view.schema()

        # View should have no component columns
        assert len(schema.component_columns()) == 0

    def test_filter_component_types_invalid_string_format(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test that short component type names raise ValueError."""
        with pytest.raises(ValueError, match="must be fully-qualified"):
            test_dataset_with_archetypes.filter_component_types("Position3D")

    def test_filter_component_types_chain_with_segments(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test chaining filter_component_types with filter_segments."""
        from rerun import components

        # Get first segment
        all_segments = sorted(test_dataset_with_archetypes.segment_ids())
        assert len(all_segments) >= 1

        first_segment = all_segments[0]

        # Chain filters
        view = (
            test_dataset_with_archetypes.filter_segments([first_segment])
            .filter_component_types(components.Position3D)
        )

        # Should have filtered by both segment and component type
        assert view.segment_ids() == [first_segment]
        schema = view.schema()
        component_types = schema.component_types()
        assert "rerun.components.Position3D" in component_types

    def test_filter_component_types_chain_with_contents(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test chaining filter_component_types with filter_contents."""
        from rerun import components

        # Filter by entity path and component type
        view = test_dataset_with_archetypes.filter_contents(
            ["/world/points"]
        ).filter_component_types(components.Position3D)

        schema = view.schema()

        # Should only have Position3D at /world/points
        component_columns = schema.component_columns()
        assert len(component_columns) > 0
        for col in component_columns:
            assert col.component_type == "rerun.components.Position3D"
            assert col.entity_path == "/world/points"

    def test_filter_component_types_chain_with_archetypes(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test chaining filter_component_types with filter_archetypes."""
        from rerun import components

        # Filter by archetype then component type
        # This should give us only Position3D components from Points3D archetype
        view = test_dataset_with_archetypes.filter_archetypes(
            rr.Points3D
        ).filter_component_types(components.Position3D)

        schema = view.schema()

        # Should have Position3D component type
        component_types = schema.component_types()
        assert "rerun.components.Position3D" in component_types

        # All columns should be from Points3D archetype
        component_columns = schema.component_columns()
        assert len(component_columns) > 0
        for col in component_columns:
            assert col.archetype == "rerun.archetypes.Points3D"
            assert col.component_type == "rerun.components.Position3D"


class TestDatasetViewFiltering:
    """Test suite for DatasetView.filter_archetypes() and filter_component_types()."""

    def test_datasetview_filter_archetypes(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test that DatasetView also has filter_archetypes method."""
        # First create a view
        view1 = test_dataset_with_archetypes.filter_archetypes(rr.Points3D)

        # Then filter again on the view
        view2 = view1.filter_archetypes(rr.Points3D)

        # Both should have the same schema
        assert view1.schema() == view2.schema()

    def test_datasetview_filter_component_types(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test that DatasetView also has filter_component_types method."""
        from rerun import components

        # First create a view
        view1 = test_dataset_with_archetypes.filter_component_types(
            components.Position3D
        )

        # Then filter again on the view
        view2 = view1.filter_component_types(components.Position3D)

        # Both should have the same schema
        assert view1.schema() == view2.schema()

    def test_datasetview_multiple_filters(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test chaining multiple filters on DatasetView."""
        from rerun import components

        # Get first segment
        all_segments = sorted(test_dataset_with_archetypes.segment_ids())
        first_segment = all_segments[0]

        # Chain multiple filters
        view = (
            test_dataset_with_archetypes.filter_segments([first_segment])
            .filter_archetypes(rr.Points3D)
            .filter_component_types(components.Position3D)
            .filter_contents(["/world/points"])
        )

        # All filters should be applied
        assert view.segment_ids() == [first_segment]

        schema = view.schema()
        component_columns = schema.component_columns()
        assert len(component_columns) > 0

        for col in component_columns:
            assert col.entity_path == "/world/points"
            assert col.archetype == "rerun.archetypes.Points3D"
            assert col.component_type == "rerun.components.Position3D"


class TestDataAccess:
    """Test that filtered views can actually read data."""

    def test_filter_archetypes_with_reader(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test reading data through a filtered view."""
        view = test_dataset_with_archetypes.filter_archetypes(rr.Points3D)

        # Should be able to read from the filtered view
        df = view.reader(index="frame")
        assert df is not None

        # Check that we can collect the data
        batches = df.collect()
        assert len(batches) > 0

    def test_filter_component_types_with_reader(
        self, test_dataset_with_archetypes: DatasetEntry
    ) -> None:
        """Test reading data through a component type filtered view."""
        from rerun import components

        view = test_dataset_with_archetypes.filter_component_types(
            components.Position3D
        )

        # Should be able to read from the filtered view
        df = view.reader(index="frame")
        assert df is not None

        # Check that we can collect the data
        batches = df.collect()
        assert len(batches) > 0
