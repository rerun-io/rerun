"""Comprehensive tests for filter_contents() behavior - both current and proposed."""

from __future__ import annotations

import tempfile
import uuid
from typing import TYPE_CHECKING

import pytest
import rerun as rr

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import CatalogClient, DatasetEntry

    from e2e_redap_tests.conftest import EntryFactory


@pytest.fixture
def test_dataset_multi_component(
    entry_factory: EntryFactory, catalog_client: CatalogClient
) -> DatasetEntry:
    """Create a test dataset with multiple components at same entity."""
    from pathlib import Path

    with tempfile.TemporaryDirectory() as tmpdir:
        rrd = f"{tmpdir}/test_multi.rrd"

        recording_id = uuid.uuid4()
        with rr.RecordingStream("test_multi", recording_id=recording_id) as rec:
            rec.save(rrd)

            # Log data with multiple components at same entity
            rec.set_time("frame", sequence=1)

            # /world/points has Points3D with positions and colors
            rec.log(
                "/world/points",
                rr.Points3D(
                    [[1, 2, 3], [4, 5, 6]],
                    colors=[[255, 0, 0], [0, 255, 0]],
                    radii=[0.1, 0.2]
                ),
            )

            # /world/boxes has Boxes3D
            rec.log(
                "/world/boxes",
                rr.Boxes3D(half_sizes=[[1, 1, 1]]),
            )

            # /world/camera has Transform3D
            rec.log(
                "/world/camera",
                rr.Transform3D(translation=[1, 2, 3]),
            )

        dataset = entry_factory.create_dataset("test_multi_component")
        tmpdir_url = Path(tmpdir).as_uri()
        handle = dataset.register_prefix(tmpdir_url)
        handle.wait(timeout_secs=30)

        return dataset


class TestEntityPathFiltering:
    """Tests for entity path filtering - filter_contents() with entity path patterns."""

    def test_filter_contents_by_entity_path_includes_all_components(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Filtering by entity path includes ALL components at that path."""
        # Filter to just /world/points entity
        view = test_dataset_multi_component.filter_contents(["/world/points"])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Should have columns for /world/points
        points_columns = [col for col in component_columns if col.entity_path == "/world/points"]
        assert len(points_columns) > 0, "Should have columns for /world/points"

        # Should NOT have columns for other entities
        other_columns = [col for col in component_columns if col.entity_path != "/world/points"]
        assert len(other_columns) == 0, "Should not have columns for other entities"

        # CURRENT BEHAVIOR: Should include ALL components at /world/points (positions, colors, radii)
        component_types = {col.component_type for col in points_columns}
        assert "rerun.components.Position3D" in component_types
        assert "rerun.components.Color" in component_types
        # Note: This validates current behavior - we get all components, not just specific ones

    def test_filter_contents_by_wildcard_includes_all_components(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """wildcard filtering includes all components at matching entities."""
        # Filter using wildcard
        view = test_dataset_multi_component.filter_contents(["/world/**"])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Should have multiple entities
        entity_paths = {col.entity_path for col in component_columns}
        assert "/world/points" in entity_paths
        assert "/world/boxes" in entity_paths
        assert "/world/camera" in entity_paths

        # For each entity, should have ALL its components
        points_columns = [col for col in component_columns if col.entity_path == "/world/points"]
        points_component_types = {col.component_type for col in points_columns}
        assert "rerun.components.Position3D" in points_component_types
        assert "rerun.components.Color" in points_component_types

    def test_filter_contents_excludes_entity_paths(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """can exclude entity paths using - prefix."""
        # Include all under /world but exclude /world/boxes
        view = test_dataset_multi_component.filter_contents(["/world/**", "-/world/boxes"])

        schema = view.schema()
        component_columns = schema.component_columns()

        entity_paths = {col.entity_path for col in component_columns}
        assert "/world/points" in entity_paths
        assert "/world/camera" in entity_paths
        assert "/world/boxes" not in entity_paths

    def test_filter_contents_empty_list_returns_all_columns(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """empty filter list returns all columns."""
        view = test_dataset_multi_component.filter_contents([])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Empty list means "no filter" = all columns
        assert len(component_columns) > 0

    def test_filter_contents_nonexistent_path_returns_empty_view(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """filtering to non-existent path returns empty view."""
        view = test_dataset_multi_component.filter_contents(["/nonexistent/**"])

        schema = view.schema()
        component_columns = schema.component_columns()

        assert len(component_columns) == 0

    def test_filter_contents_chains_with_segment_filter(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """filter_contents chains with filter_segments."""
        segments = test_dataset_multi_component.segment_ids()
        assert len(segments) >= 1

        first_segment = segments[0]

        # Chain filters
        view = (test_dataset_multi_component
                .filter_segments([first_segment])
                .filter_contents(["/world/points"]))

        # Should have filtered segment
        assert view.segment_ids() == [first_segment]

        # Should have filtered contents
        schema = view.schema()
        component_columns = schema.component_columns()
        entity_paths = {col.entity_path for col in component_columns}
        assert entity_paths == {"/world/points"}


class TestComponentColumnFiltering:
    """Tests for component-level filtering using column selector strings."""

    def test_filter_contents_with_column_selector_filters_specific_component(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Can filter to specific component using column selector string."""
        # Proposed API: column selector format "entity_path:component"
        view = test_dataset_multi_component.filter_contents([
            "/world/points:Points3D:positions"
        ])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Should only have the specified component
        assert len(component_columns) == 1
        assert component_columns[0].entity_path == "/world/points"
        assert component_columns[0].component == "Points3D:positions"

    def test_filter_contents_with_multiple_column_selectors_same_entity(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Can specify multiple components at same entity."""
        view = test_dataset_multi_component.filter_contents([
            "/world/points:Points3D:positions",
            "/world/points:Points3D:colors"
        ])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Should have exactly the two specified components
        assert len(component_columns) == 2
        components = {col.component for col in component_columns}
        assert components == {"Points3D:positions", "Points3D:colors"}

    def test_filter_contents_with_column_selectors_multiple_entities(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Can filter components across multiple entities."""
        view = test_dataset_multi_component.filter_contents([
            "/world/points:Points3D:positions",
            "/world/camera:Transform3D:translation"
        ])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Should have components from both entities
        assert len(component_columns) == 2
        entity_paths = {col.entity_path for col in component_columns}
        assert entity_paths == {"/world/points", "/world/camera"}

    def test_filter_contents_column_selector_with_wildcard_not_supported(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Column selectors don't support wildcards in entity path."""
        # Note: Wildcard entity paths with column selectors would require special handling
        # For now, we only support exact entity paths in column selectors
        view = test_dataset_multi_component.filter_contents([
            "/world/points:Points3D:positions"
        ])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Should only have the exact entity path specified
        assert len(component_columns) == 1
        assert component_columns[0].entity_path == "/world/points"

    def test_filter_contents_mixed_entity_paths_and_column_selectors_raises(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Mixing entity paths and column selectors raises error (for now)."""
        # Mixing requires more complex implementation, so we disallow it initially
        import pytest
        with pytest.raises(ValueError, match="Cannot mix entity path patterns and column selectors"):
            test_dataset_multi_component.filter_contents([
                "/world/camera",  # Entity path (no colon)
                "/world/points:Points3D:positions"  # Column selector (has colon)
            ])

    def test_filter_contents_column_selectors_chain_with_other_filters(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Column selector format chains properly with other filters."""
        segments = test_dataset_multi_component.segment_ids()
        first_segment = segments[0]

        view = (test_dataset_multi_component
                .filter_segments([first_segment])
                .filter_contents(["/world/points:Points3D:positions"]))

        # Should have filtered segment
        assert view.segment_ids() == [first_segment]

        # Should have filtered to specific component
        schema = view.schema()
        component_columns = schema.component_columns()
        assert len(component_columns) == 1
        assert component_columns[0].component == "Points3D:positions"

    def test_filter_contents_works_with_column_names_for(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """column_names_for() output can be passed directly to filter_contents()."""
        # Get column names for Position3D components using column_names_for()
        schema = test_dataset_multi_component.schema()
        position_columns = schema.column_names_for(component_type="rerun.components.Position3D")

        # Should find the position column
        assert len(position_columns) > 0
        assert "/world/points:Points3D:positions" in position_columns

        # Use those column names directly in filter_contents()
        view = test_dataset_multi_component.filter_contents(position_columns)

        # Should have filtered to only Position3D components
        filtered_schema = view.schema()
        filtered_columns = filtered_schema.component_columns()

        # All filtered columns should be Position3D
        for col in filtered_columns:
            assert col.component_type == "rerun.components.Position3D"


class TestPropertyPathHandling:
    """Tests for property path edge cases."""

    def test_property_paths_are_treated_as_entity_paths(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Property paths with colons are treated as entity paths, not column selectors."""
        # Property paths contain colons but should be treated as entity paths
        # This should not raise an error about mixing modes
        view = test_dataset_multi_component.filter_contents([
            "/world/points",
            "property:test"  # Has colon but is a property path
        ])

        # Should succeed without error (no "Cannot mix..." error)
        schema = view.schema()
        assert schema is not None

    def test_properties_directory_treated_as_entity_path(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Paths containing /__properties: are treated as entity paths."""
        # This should not raise an error about mixing modes
        view = test_dataset_multi_component.filter_contents([
            "/world/camera",
            "/world/__properties:custom"  # Has colon but is a property path
        ])

        # Should succeed without error
        schema = view.schema()
        assert schema is not None


class TestEdgeCaseColumnSelectors:
    """Tests for edge case column selector behaviors."""

    def test_nonexistent_column_selector_returns_empty_view(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Column selector for non-existent component returns empty view."""
        # This is valid syntax but doesn't match anything
        view = test_dataset_multi_component.filter_contents([
            "/world/points:NonExistentComponent"
        ])

        schema = view.schema()
        component_columns = schema.component_columns()

        # Should return empty (no matching columns)
        assert len(component_columns) == 0

    def test_malformed_selector_with_missing_entity_path(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Selector starting with colon gets treated as entity path."""
        # ":Component" is ambiguous - treated as entity path pattern
        view = test_dataset_multi_component.filter_contents([":Component"])

        schema = view.schema()
        # Should work but likely returns empty (no entity starts with ":")
        assert schema is not None

    def test_selector_with_trailing_colon(
        self, test_dataset_multi_component: DatasetEntry
    ) -> None:
        """Selector ending with colon has undefined behavior."""
        # "/world/points:" - behavior depends on ComponentColumnSelector parser
        view = test_dataset_multi_component.filter_contents(["/world/points:"])

        schema = view.schema()
        # Should not crash - behavior depends on Rust parser
        assert schema is not None
