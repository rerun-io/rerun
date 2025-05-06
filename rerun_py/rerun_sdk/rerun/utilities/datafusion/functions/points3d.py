from __future__ import annotations

import rerun_bindings
from datafusion import DataFrame, Expr, ScalarUDF, col, udf

from .columns import column_for_component


def convert_depth_image_to_point_cloud_by_path(
    df: DataFrame, depth_image_path: str, pinhole_path: str, output_path: str, remove_nulls: bool = True
) -> DataFrame:
    """
    Convert a depth image to a point cloud.

    This is a Rerun DataFusion function that will convert from a DepthImage and Pinhole
    into a Points3D.

    To explicitly control which data columns to use with this function, see
    `convert_depth_image_to_point_cloud_by_col`.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    depth_image_path:
        Entity path for the DepthImage
    pinhole_path:
        Entity path for the Pinhole
    output_path:
        Entity path to output
    remove_nulls:
        If True, performs a filter operation on the resultant DataFrame

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to the components of
    the Points3D archetype.

    """
    schema = df.schema()
    prior_columns = schema.names

    image_buffer_col = col(column_for_component(schema, depth_image_path, "ImageBuffer", archetype="DepthImage"))
    image_format_col = col(column_for_component(schema, depth_image_path, "ImageFormat", archetype="DepthImage"))
    depth_meter_col = col(column_for_component(schema, depth_image_path, "DepthMeter", archetype="DepthImage"))
    pinhole_projection_col = col(column_for_component(schema, pinhole_path, "PinholeProjection", archetype="Pinhole"))
    resolution_col = col(column_for_component(schema, pinhole_path, "Resolution", archetype="Pinhole"))

    indicator_output = f"{output_path}:Points3DIndicator"
    position_output = f"{output_path}:Position3D"

    df = convert_depth_image_to_point_cloud_by_col(
        df,
        image_buffer_col,
        image_format_col,
        depth_meter_col,
        pinhole_projection_col,
        resolution_col,
        indicator_output,
        position_output,
        remove_nulls=remove_nulls,
    )

    return df.select(
        *prior_columns,
        col(indicator_output).alias(indicator_output, metadata={"rerun.entity_path": output_path}),
        col(position_output).alias(position_output, metadata={"rerun.entity_path": output_path}),
    )


def convert_depth_image_to_point_cloud_by_col(
    df: DataFrame,
    image_buffer_column: str | Expr,
    image_format_column: str | Expr,
    depth_meter_column: str | Expr,
    pinhole_projection_column: str | Expr,
    resolution_column: str | Expr,
    indicator_output_column: str,
    position_output_column: str,
    remove_nulls: bool = True,
) -> DataFrame:
    """
    Convert a depth image to a point cloud.

    This is a Rerun DataFusion function that will convert from a DepthImage and Pinhole
    into a Points3D.

    To manually invoke the underlying UDF, see `convert_depth_image_to_point_cloud_udf`.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    image_buffer_column:
        Column containing the depth image buffer
    image_format_column:
        Column containing the depth image format
    depth_meter_column:
        Column containing the depth meter
    pinhole_projection_column:
        Column containing the pinhole projection
    resolution_column:
        Column containing the pinhole resolution
    indicator_output_column:
        Output column name for the Points3D indicator
    position_output_column:
        Output column name for the Points3D positions
    remove_nulls:
        If True, performs a filter operation on the resultant DataFrame

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to the components of
    the Points3D archetype.

    """
    image_buffer_column = col(image_buffer_column) if isinstance(image_buffer_column, str) else image_buffer_column
    image_format_column = col(image_format_column) if isinstance(image_format_column, str) else image_format_column
    depth_meter_column = col(depth_meter_column) if isinstance(depth_meter_column, str) else depth_meter_column
    pinhole_projection_column = (
        col(pinhole_projection_column) if isinstance(pinhole_projection_column, str) else pinhole_projection_column
    )
    resolution_column = col(resolution_column) if isinstance(resolution_column, str) else resolution_column

    prior_columns = df.schema().names
    temp_path = f"{position_output_column}_udf"
    depth_to_cloud_udf = convert_depth_image_to_point_cloud_udf()

    df = df.select(
        *prior_columns,
        depth_to_cloud_udf(
            image_buffer_column,
            image_format_column,
            depth_meter_column,
            pinhole_projection_column,
            resolution_column,
        ).alias(temp_path),
    )

    if remove_nulls:
        df = df.filter(col(temp_path)["Position3D"].is_not_null())

    return df.select(
        *prior_columns,
        col(temp_path)["Points3DIndicator"].alias(indicator_output_column),
        col(temp_path)["Position3D"].alias(position_output_column),
    )


def convert_depth_image_to_point_cloud_udf() -> ScalarUDF:
    return udf(rerun_bindings.datafusion.depth_image_to_point_cloud_udf())
