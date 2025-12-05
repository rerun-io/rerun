from __future__ import annotations

import itertools
from typing import TYPE_CHECKING

from rerun.dataframe import ComponentColumnDescriptor, ComponentColumnSelector, IndexColumnDescriptor

if TYPE_CHECKING:
    from collections.abc import Iterator

    from rerun_bindings import SchemaInternal


class Schema:
    """
    The schema representing a set of available columns for a dataset.

    A schema contains both index columns (timelines) and component columns (entity/component data).

    This class wraps the internal schema representation and provides a Pythonic interface
    for inspecting the available columns in a dataset.
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
        self._component_columns: list[ComponentColumnDescriptor] = []
        self._index_columns: list[IndexColumnDescriptor] = []

        for col in self._internal:
            if isinstance(col, ComponentColumnDescriptor):
                self._component_columns.append(col)
            elif isinstance(col, IndexColumnDescriptor):
                self._index_columns.append(col)

    def __iter__(self) -> Iterator[IndexColumnDescriptor | ComponentColumnDescriptor]:
        """Iterate over all column descriptors in the schema (index columns first, then component columns)."""
        return itertools.chain(self._index_columns, self._component_columns)

    def index_columns(self) -> list[IndexColumnDescriptor]:
        """
        Return a list of all the index columns in the schema.

        Index columns contain the index values for when the data was updated.
        They generally correspond to Rerun timelines.
        """
        return self._index_columns

    def component_columns(self) -> list[ComponentColumnDescriptor]:
        """
        Return a list of all the component columns in the schema.

        Component columns contain the data for a specific component of an entity.
        """
        return self._component_columns

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
        for col in self._component_columns:
            if col.entity_path == entity_path and col.component == component:
                return col
        return None

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
        KeyError
            If the column is not found.
        ValueError
            If the string selector format is invalid.

        """
        if isinstance(selector, ComponentColumnDescriptor):
            return selector
        if isinstance(selector, ComponentColumnSelector):
            entity_path = selector.entity_path
            component = selector.component
        else:  # str
            parts = selector.split(":")
            if len(parts) != 2:
                raise ValueError(f"Invalid selector format: {selector}. Expected '<entity_path>:<component>'")
            entity_path, component = parts

        result = self.column_for(entity_path, component)
        if result is None:
            raise KeyError(f"Column not found for selector: {selector}")
        return result

    def column_names(self) -> list[str]:
        """
        Return a list of all column names in the schema.

        Returns
        -------
        list[str]
            The names of all columns (index columns first, then component columns).

        """
        return [col.name for col in self]

    def __repr__(self) -> str:
        """Return a string representation of the schema."""
        return "\n".join(repr(col) for col in self)

    def __eq__(self, other: object) -> bool:
        """Check equality with another Schema."""
        if not isinstance(other, Schema):
            return NotImplemented
        return self._index_columns == other._index_columns and self._component_columns == other._component_columns
