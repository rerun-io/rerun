from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence

import pyarrow as pa

from ... import datatypes

if TYPE_CHECKING:
    from .component_column_selector import ComponentColumnSelectorArrayLike


class ComponentColumnSelectorExt:
    """Extension for [ComponentColumnSelector][rerun.blueprint.datatypes.ComponentColumnSelector]."""

    def __init__(
        self: Any,
        spec: str | None = None,
        *,
        entity_path: datatypes.EntityPathLike | None = None,
        component: datatypes.Utf8Like | None = None,
    ):
        """
        Create a new instance of the ComponentColumnSelector datatype.

        Parameters
        ----------
        spec:
            A string in the format "/entity/path:ComponentName". If used, `entity_path` and `component` must be `None`.

        entity_path:
            The column's entity path. If used, `spec` must be `None` and `component` must be provided.

        component:
            The column's component name. If used, `spec` must be `None` and `entity_path` must be provided.

        """

        if spec is not None:
            if entity_path is not None or component is not None:
                raise ValueError("Either `spec` or both `entity_path` and `component` must be provided.")
            if not isinstance(spec, str):
                raise ValueError(f"Unexpected input value (`spec` must be a string): {spec}")
            entity_path, component = _parse_spec(spec)
        else:
            if entity_path is None or component is None:
                raise ValueError("Both `entity_path` and `component` must be provided.")

        self.__attrs_init__(entity_path=entity_path, component=component)

    # Override needed to address the `str` case.
    @staticmethod
    def native_to_pa_array_override(input_data: ComponentColumnSelectorArrayLike, data_type: pa.DataType) -> pa.Array:
        from ...components import EntityPathBatch
        from ...datatypes import Utf8Batch
        from .component_column_selector import ComponentColumnSelector

        if isinstance(input_data, ComponentColumnSelector):
            data: Sequence[ComponentColumnSelector] = [input_data]
        else:
            data = [
                item if isinstance(item, ComponentColumnSelector) else ComponentColumnSelector(item)
                for item in input_data
            ]

        return pa.StructArray.from_arrays(
            [
                EntityPathBatch([x.entity_path for x in data]).as_arrow_array().storage,  # type: ignore[misc, arg-type]
                Utf8Batch([x.component for x in data]).as_arrow_array().storage,  # type: ignore[misc, arg-type]
            ],
            fields=list(data_type),
        )


def _parse_spec(spec: str) -> tuple[datatypes.EntityPath, datatypes.Utf8]:
    """
    Parse the component column specifier.

    Raises `ValueError` if the specifier is invalid.
    """

    try:
        entity_path, component = spec.split(":")
    except ValueError as e:
        raise ValueError(f"Invalid component column specifier: {spec}") from e

    return datatypes.EntityPath(entity_path), datatypes.Utf8(component)
