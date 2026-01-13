from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from collections.abc import Iterator, Sequence

    from rerun_bindings import (
        ComponentColumnDescriptor,
        ComponentColumnSelector,
        IndexColumnDescriptor,
        SchemaInternal,
    )


class Schema:
    """
    The schema representing a set of available columns for a dataset.

    A schema contains both index columns (timelines) and component columns (entity/component data).
    """

    def __init__(self, inner: SchemaInternal) -> None:
        """
        Create a new Schema wrapper.

        Parameters
        ----------
        inner : SchemaInternal
            The internal schema object from the bindings.

        """
        self._internal = inner

    def __iter__(self) -> Iterator[IndexColumnDescriptor | ComponentColumnDescriptor]:
        """Iterate over all column descriptors in the schema (index columns first, then component columns)."""

        # TODO(#9922): we should support control columns like row id as well.
        return itertools.chain(self.index_columns(), self.component_columns())

    def index_columns(self) -> Sequence[IndexColumnDescriptor]:
        """
        Return a list of all the index columns in the schema.

        Index columns contain the index values for when the data was updated.
        They generally correspond to Rerun timelines.
        """
        return self._internal.index_columns()

    def component_columns(self) -> Sequence[ComponentColumnDescriptor]:
        """
        Return a list of all the component columns in the schema.

        Component columns contain the data for a specific component of an entity.
        """
        return self._internal.component_columns()

    def column_for(self, entity_path: str, component: str) -> ComponentColumnDescriptor | None:
        """
        Look up the column descriptor for a specific entity path and component.

        Parameters
        ----------
        entity_path : str
            The entity path to look up.
        component : str
            The component to look up. Example: `Points3D:positions`.

        Returns
        -------
        ComponentColumnDescriptor | None
            The column descriptor, if it exists.

        """
        return self._internal.column_for(entity_path, component)

    def column_for_selector(
        self, selector: str | ComponentColumnSelector | ComponentColumnDescriptor
    ) -> ComponentColumnDescriptor:
        """
        Look up the column descriptor for a specific selector.

        Parameters
        ----------
        selector : str | ComponentColumnSelector | ComponentColumnDescriptor
            The selector to look up.

            String arguments are expected to follow the format: `"<entity_path>:<component_type>"`

        Returns
        -------
        ComponentColumnDescriptor
            The column descriptor.

        Raises
        ------
        LookupError
            If the column is not found.
        ValueError
            If the string selector format is invalid or the input type is unsupported.

        Note: if the input is already a `ComponentColumnDescriptor`, it is
        returned directly without checking for existence.

        """
        return self._internal.column_for_selector(selector)

    def column_names(self) -> list[str]:
        """
        Return a list of all column names in the schema.

        Returns
        -------
        The names of all columns (index columns first, then component columns).

        """
        return [col.name for col in self]

    def archetypes(self) -> list[str]:
        """
        List all unique archetype names in the schema.

        Returns a sorted list of fully-qualified archetype names.

        Returns
        -------
        list[str]
            Sorted list of archetype names (e.g., ["rerun.archetypes.Points3D", ...])

        Examples
        --------
        ```python
        schema = dataset.schema()
        archetypes = schema.archetypes()
        print(archetypes)
        # ['rerun.archetypes.Boxes3D', 'rerun.archetypes.Points3D', 'rerun.archetypes.Transform3D']
        ```

        """
        return self._internal.archetypes()

    def entities(self) -> list[str]:
        """
        List all unique entity paths in the schema.

        Returns a sorted list of entity paths.

        Returns
        -------
        list[str]
            Sorted list of entity paths (e.g., ["/world/points", "/world/camera", ...])

        Examples
        --------
        ```python
        schema = dataset.schema()
        entities = schema.entities()
        print(entities)
        # ['/world/boxes', '/world/camera', '/world/points']
        ```

        """
        return self._internal.entities()

    def component_types(self) -> list[str]:
        """
        List all unique component type names in the schema.

        Returns a sorted list of fully-qualified component type names.

        Returns
        -------
        list[str]
            Sorted list of component type names (e.g., ["rerun.components.Position3D", …])

        Examples
        --------
        ```python
        schema = dataset.schema()
        component_types = schema.component_types()
        print(component_types)
        # ['rerun.components.Color', 'rerun.components.HalfSize3D', 'rerun.components.Position3D', …]
        ```

        """
        return self._internal.component_types()

    def columns_for(
        self,
        entity_path: str | None = None,
        archetype: str | None = None,
        component_type: str | None = None,
    ) -> list[ComponentColumnDescriptor]:
        """
        Get columns matching ALL specified criteria.

        Filters columns using AND logic - a column must match all non-None criteria.

        Parameters
        ----------
        entity_path : str | None
            Optional entity path filter.
        archetype : str | None
            Optional archetype name filter (fully-qualified, e.g., "rerun.archetypes.Points3D").
        component_type : str | None
            Optional component type filter (fully-qualified, e.g., "rerun.components.Position3D").

        Returns
        -------
        list[ComponentColumnDescriptor]
            List of columns matching all provided criteria.

        Examples
        --------
        ```python
        schema = dataset.schema()

        # Get all Position3D components
        position_cols = schema.columns_for(component_type="rerun.components.Position3D")

        # Get all columns at a specific entity
        point_cols = schema.columns_for(entity_path="/world/points")

        # Get specific component at specific entity
        specific = schema.columns_for(
            entity_path="/world/points",
            component_type="rerun.components.Position3D"
        )
        ```

        """
        return self._internal.columns_for(entity_path, archetype, component_type)

    def column_names_for(
        self,
        entity_path: str | None = None,
        archetype: str | None = None,
        component_type: str | None = None,
    ) -> list[str]:
        """
        Get column names matching ALL specified criteria.

        This is a convenience method that returns just the column names,
        designed for use with `dataset.filter_contents()`.

        Parameters
        ----------
        entity_path : str | None
            Optional entity path filter.
        archetype : str | None
            Optional archetype name filter (fully-qualified, e.g., "rerun.archetypes.Points3D").
        component_type : str | None
            Optional component type filter (fully-qualified, e.g., "rerun.components.Position3D").

        Returns
        -------
        list[str]
            List of column names matching all provided criteria.

        Examples
        --------
        ```python
        schema = dataset.schema()

        # Get column names for all Position3D components
        position_names = schema.column_names_for(component_type="rerun.components.Position3D")

        # Use with filter_contents() to filter dataset
        view = dataset.filter_contents(position_names)

        # Get all Points3D archetype columns
        points_names = schema.column_names_for(archetype="rerun.archetypes.Points3D")
        ```

        """
        return self._internal.column_names_for(entity_path, archetype, component_type)

    def __repr__(self) -> str:
        """Return a string representation of the schema."""
        return "\n".join(repr(col) for col in self)

    def __eq__(self, other: object) -> bool:
        """Check equality with another Schema."""
        if not isinstance(other, Schema):
            return NotImplemented
        # Impl note: this delegates to the `Eq` trait of `PySchemaInternal`
        return self._internal == other._internal

    def __arrow_c_schema__(self) -> Any:
        return self._internal.__arrow_c_schema__()
