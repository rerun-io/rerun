from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import DataFrame, col

if TYPE_CHECKING:
    import pyarrow as pa


def column_for_component(
    schema: pa.Schema | DataFrame,
    entity_path: str | None,
    component: str | None,
    archetype: str | None = None,
) -> str | None:
    """
    Find the column name based on metadata.

    This function will scan the metadata in a schema to identify the column name
    for a Rerun component. The user can specify the entity path, component type,
    and archetype. All fields are optional. The first matching column found will
    be returned.

    Parameters
    ----------
    schema:
        The input schema.
    entity_path:
        The name of Rerun entity path to find.
    component:
        The name of the Rerun component to find.
    archetype:
        The name of the Rerun archetype to find.

    Returns
    -------
    A column name corresponding to the search criteria, if found.

    """
    if isinstance(schema, DataFrame):
        schema = schema.schema()
    for field in schema:
        valid_path = True
        valid_archetype = True
        valid_component = True
        if entity_path is not None:
            valid_path = False
            if b"rerun.entity_path" in field.metadata.keys():
                if field.metadata[b"rerun.entity_path"].decode("utf-8") == entity_path:
                    valid_path = True
        if archetype is not None:
            valid_archetype = False
            if b"rerun.archetype" in field.metadata.keys():
                if field.metadata[b"rerun.archetype"].decode("utf-8") == f"rerun.archetypes.{archetype}":
                    valid_archetype = True
        if component is not None:
            valid_component = False
            if b"rerun.component" in field.metadata.keys():
                if field.metadata[b"rerun.component"].decode("utf-8") == f"rerun.components.{component}":
                    valid_component = True
        if valid_path and valid_archetype and valid_component:
            return str(field.name)

    return None


def column_for_index(schema: pa.Schema | DataFrame, index: str) -> str | None:
    """
    Find the column name for an index based on metadata.

    This function will scan the metadata in a schema to identify the column name
    for a Rerun index.

    Parameters
    ----------
    schema:
        The input schema.
    index:
        The name of the index to search for

    Returns
    -------
    A column name corresponding to the search criteria, if found.

    """
    if isinstance(schema, DataFrame):
        schema = schema.schema()
    for field in schema:
        if b"rerun.index_name" in field.metadata.keys():
            if field.metadata[b"rerun.index_name"].decode("utf-8") == index:
                return str(field.name)
    return None


def duplicate_components(
    df: DataFrame, input_path: str, output_path: str, components: list[str] | list[tuple[str, str]]
) -> DataFrame:
    """
    Duplicate components from one entity path to another.

    In addition to duplicating the data from one column to another, this sets
    the appropriate Rerun metadata for the entity path on the output.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    input_path:
        Entity path to copy components from
    output_path:
        Entity path to copy components to
    components:
        List of components to copy. This can be either a list of strings
        in which we assume there will only be one component for that entity,
        or it can be a list of tuples of (Archetype, Component).

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to duplicates
    of the original columns. The new columns will have names
    `output_path:component_name`.

    """
    schema = df.schema()
    for component in components:
        if isinstance(component, str):
            col_name = column_for_component(schema, entity_path=input_path, component=component)
        else:
            col_name = column_for_component(
                schema, entity_path=input_path, archetype=component[0], component=component[1]
            )

        df = df.with_column(
            f"{output_path}:{component}",
            col(col_name).alias(f"{output_path}:{component}", metadata={"rerun.entity_path": output_path}),
        )

    return df
