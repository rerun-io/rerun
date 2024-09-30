from __future__ import annotations

from ... import datatypes


class ComponentColumnSelectorExt:
    """Extension for [ComponentColumnSelector][rerun.blueprint.components.ComponentColumnSelector]."""

    def __init__(
        self,
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
            entity_path, component = _parse_spec(spec)
        else:
            if entity_path is None or component is None:
                raise ValueError("Both `entity_path` and `component` must be provided.")

        super().__init__(entity_path=entity_path, component=component)


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
