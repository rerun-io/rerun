from __future__ import annotations

import rerun_bindings
from datafusion import DataFrame, Expr, ScalarUDF, col, udf

from .columns import column_for_component


def intersection_over_union_by_path(
    df: DataFrame,
    entity_1_path: str,
    entity_2_path: str,
    output_path: str,
    remove_nulls: bool = True,
) -> DataFrame:
    """
    Compute the intersection over union of two boxes.

    This is a Rerun focused DataFusion function that will assume there are two
    entities that contain Boxes2D archetypes. It will identify the corresponding
    data columns and perform a calculation of the area of the intersection of the
    boxes divided by the area of the union of the boxes. For boxes that exactly overlap
    this should yield a scalar value of 1.0 and for boxes that do not intersect it
    will yield a value of 0.0. If either of the columns for the boxes is null, it
    will yield a null.

    To explicitly control which data columns to use with this function, see
    `intersection_over_union_by_col`.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    entity_1_path:
        Entity path for the input first Boxes2D components.
    entity_2_path:
        Entity path for the input second Boxes2D components.
    output_path:
        Entity path to output
    remove_nulls:
        If True, performs a filter operation on the resultant DataFrame

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to the components of
    the Image archetype.

    """
    schema = df.schema()
    prior_columns = schema.names

    ent1_pos_col = column_for_component(schema, entity_1_path, "Position2D", archetype="Boxes2D")
    ent1_half_size_col = column_for_component(schema, entity_1_path, "HalfSize2D", archetype="Boxes2D")
    ent2_pos_col = column_for_component(schema, entity_2_path, "Position2D", archetype="Boxes2D")
    ent2_half_size_col = column_for_component(schema, entity_2_path, "HalfSize2D", archetype="Boxes2D")

    output_col = f"{output_path}:Scalar"

    df = intersection_over_union_by_col(
        df, ent1_pos_col, ent1_half_size_col, ent2_pos_col, ent2_half_size_col, output_col, remove_nulls
    )

    return df.select(
        *prior_columns,
        col(output_col).alias(f"{output_path}:Scalar", metadata={"rerun.entity_path": output_path}),
    )


def intersection_over_union_by_col(
    df: DataFrame,
    box_1_position_column: str | Expr,
    box_1_half_size_column: str | Expr,
    box_2_position_column: str | Expr,
    box_2_half_size_column: str | Expr,
    output_column: str,
    remove_nulls: bool = True,
) -> DataFrame:
    """
    Compute the intersection over union of two boxes.

    This is a Rerun focused DataFusion function that will assume there are two
    entities that contain Boxes2D archetypes. It will identify the corresponding
    data columns and perform a calculation of the area of the intersection of the
    boxes divided by the area of the union of the boxes. For boxes that exactly overlap
    this should yield a scalar value of 1.0 and for boxes that do not intersect it
    will yield a value of 0.0. If either of the columns for the boxes is null, it
    will yield a null.

    To manually invoke the underlying UDF, see `intersection_over_union_udf`.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    box_1_position_column:
        Column containing 2D position of the 1st bounding box.
    box_1_half_size_column:
        Column containing 2D half size of the 1st bounding box.
    box_2_position_column:
        Column containing 2D position of the 2nd bounding box.
    box_2_half_size_column:
        Column containing 2D half size of the 2nd bounding box.
    output_column:
        Column name for the resultant calculation.
    remove_nulls:
        If True, performs a filter operation on the resultant DataFrame

    Returns
    -------
    A DataFusion DataFrame with a new column corresponding to calculation
    of the intersection / union of the boxes

    """

    box_1_position_column = (
        col(box_1_position_column) if isinstance(box_1_position_column, str) else box_1_position_column
    )
    box_1_half_size_column = (
        col(box_1_half_size_column) if isinstance(box_1_half_size_column, str) else box_1_half_size_column
    )
    box_2_position_column = (
        col(box_2_position_column) if isinstance(box_2_position_column, str) else box_2_position_column
    )
    box_2_half_size_column = (
        col(box_2_half_size_column) if isinstance(box_2_half_size_column, str) else box_2_half_size_column
    )

    iou_udf = intersection_over_union_udf()
    df = df.with_column(
        output_column,
        iou_udf(
            box_1_position_column,
            box_1_half_size_column,
            box_2_position_column,
            box_2_half_size_column,
        ),
    )

    if remove_nulls:
        df = df.filter(col(output_column).is_not_null())

    return df


def intersection_over_union_udf() -> ScalarUDF:
    """Create a UDF to compute the intersection over union of boxes."""
    return udf(rerun_bindings.datafusion.intersection_over_union_udf())
