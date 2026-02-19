from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from collections.abc import Iterator, Sequence

    from rerun._baseclasses import Archetype
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

    def archetypes(self, *, include_properties: bool = False) -> list[str]:
        """
        Return a list of all the archetypes in the schema.

        Parameters
        ----------
        include_properties:
            If `True`, archetypes used in properties are included.

        """

        archetypes = {col.archetype for col in self.component_columns() if include_properties or not col.is_property}
        return sorted(a for a in archetypes if a is not None)

    def component_types(self, *, include_properties: bool = False) -> list[str]:
        """
        Return a list of all the component types in the schema.

        Parameters
        ----------
        include_properties:
            If `True`, component types used in properties are included.

        """

        component_types = {
            col.component_type for col in self.component_columns() if include_properties or not col.is_property
        }
        return sorted(comp for comp in component_types if comp is not None)

    def entity_paths(self, *, include_properties: bool = False) -> list[str]:
        """
        Return a sorted list of all unique entity paths in the schema. By default, the properties are not included.

        Parameters
        ----------
        include_properties:
            If `True`, include property entities (`/__properties/*`)

        """

        entity_paths = {
            col.entity_path for col in self.component_columns() if include_properties or not col.is_property
        }
        return sorted(entity_paths)

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

    def columns_for(
        self,
        *,
        entity_path: str | None = None,
        archetype: str | type[Archetype] | None = None,
        component_type: str | None = None,
        include_properties: bool = False,
    ) -> list[ComponentColumnDescriptor]:
        """
        Return a filtered list of component columns matching the given criteria.

        Parameters
        ----------
        entity_path:
            If set, only return columns with this entity path.
        archetype:
            If set, only return columns with this archetype. Accepts a fully-qualified
            archetype name (e.g., `"rerun.archetypes.Points3D"`), a short name
            (e.g., `"Points3D"`), or an Archetype class (e.g., `rr.Points3D`).
        component_type:
            If set, only return columns with this component type.
        include_properties:
            If `True`, include property columns (`/__properties/*`).

        """
        from rerun._baseclasses import Archetype

        if isinstance(archetype, type) and issubclass(archetype, Archetype):
            archetype = archetype.archetype()

        return [
            col
            for col in self.component_columns()
            if (include_properties or not col.is_property)
            and (entity_path is None or col.entity_path == entity_path)
            and (archetype is None or _match_archetype(col.archetype, archetype))
            and (component_type is None or col.component_type == component_type)
        ]

    def column_names_for(
        self,
        *,
        entity_path: str | None = None,
        archetype: str | type[Archetype] | None = None,
        component_type: str | None = None,
        include_properties: bool = False,
    ) -> list[str]:
        """
        Return a list of column names matching the given criteria.

        Parameters
        ----------
        entity_path:
            If set, only return columns with this entity path.
        archetype:
            If set, only return columns with this archetype. Accepts a fully-qualified
            archetype name (e.g., `"rerun.archetypes.Points3D"`), a short name
            (e.g., `"Points3D"`), or an Archetype class (e.g., `rr.Points3D`).
        component_type:
            If set, only return columns with this component type.
        include_properties:
            If `True`, include property columns (`/__properties/*`).

        """
        return [
            col.name
            for col in self.columns_for(
                entity_path=entity_path,
                archetype=archetype,
                component_type=component_type,
                include_properties=include_properties,
            )
        ]

    def column_names(self) -> list[str]:
        """
        Return a list of all column names in the schema.

        Returns
        -------
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
        # Impl note: this delegates to the `Eq` trait of `PySchemaInternal`
        return self._internal == other._internal

    def __arrow_c_schema__(self) -> Any:
        return self._internal.__arrow_c_schema__()


def _match_archetype(col_archetype: str | None, archetype: str) -> bool:
    if col_archetype is None:
        return False

    if "." in archetype:
        return col_archetype == archetype

    return col_archetype.rsplit(".", 1)[-1] == archetype
