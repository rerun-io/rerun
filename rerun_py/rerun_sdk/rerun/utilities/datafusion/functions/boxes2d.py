from __future__ import annotations

import rerun_bindings
from datafusion import DataFrame, Expr, ScalarUDF, col, udf

from .columns import column_for_component, column_for_index


def extract_bounding_box_images_from_video_by_path(
    df: DataFrame,
    frame_column: str,
    boxes_2d_path: str,
    asset_video_path: str,
    output_path: str,
    perform_cache: bool = True,
    remove_nulls: bool = True,
) -> DataFrame:
    """
    Extract images from a video and bounding boxes.

    This is a Rerun focused DataFusion function that will extract images from two Archetypes,
    an AssetVideo and a Boxes2D. It will return an Image.

    To explicitly control which data columns to use with this function, see
    `extract_bounding_box_images_from_video_by_col`.

    By setting `perform_cache` to True, the DataFrame will be executed when this function
    is called. This may improve performance on many systems since the video transcoding
    is a heavy process.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    frame_column:
        The name of the index that provides the frame indices to extract
    boxes_2d_path:
        Entity path for the input Boxes2D components.
    asset_video_path:
        Entity path for the input AssetVideo components.
    output_path:
        Entity path to output
    perform_cache:
        If True, performs a cache() operation after setting up the UDF.
    remove_nulls:
        If True, performs a filter operation on the resultant DataFrame

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to the components of
    the Image archetype.

    """
    schema = df.schema()
    prior_columns = schema.names

    index_col = col(column_for_index(schema, frame_column))
    position_col = col(column_for_component(schema, boxes_2d_path, "Position2D"))
    half_size_col = col(column_for_component(schema, boxes_2d_path, "HalfSize2D"))
    blob_col = col(column_for_component(schema, asset_video_path, "Blob"))
    media_type_col = col(column_for_component(schema, asset_video_path, "MediaType"))

    image_buffer_output = f"{output_path}:ImageBuffer"
    image_format_output = f"{output_path}:ImageFormat"

    df = extract_bounding_box_images_from_video_by_col(
        df,
        index_col,
        position_col,
        half_size_col,
        blob_col,
        media_type_col,
        image_buffer_output,
        image_format_output,
        perform_cache=perform_cache,
        remove_nulls=remove_nulls,
    )

    return df.select(
        *prior_columns,
        col(image_buffer_output).alias(image_buffer_output, metadata={"rerun.entity_path": output_path}),
        col(image_format_output).alias(image_format_output, metadata={"rerun.entity_path": output_path}),
    )


def extract_bounding_box_images_from_video_by_col(
    df: DataFrame,
    index_column: str | Expr,
    position_column: str | Expr,
    half_size_column: str | Expr,
    blob_column: str | Expr,
    media_type_column: str | Expr,
    image_buffer_output_column: str,
    image_format_output_column: str,
    perform_cache: bool = True,
    remove_nulls: bool = True,
) -> DataFrame:
    """
    Extract images from a video and bounding boxes.

    This is a Rerun focused DataFusion function that will extract images from two Archetypes,
    an AssetVideo and a Boxes2D. It will return an Image.

    To manually invoke the underlying UDF, see `extract_bounding_box_images_from_video_udf`.

    By setting `perform_cache` to True, the DataFrame will be executed when this function
    is called. This may improve performance on many systems since the video transcoding
    is a heavy process.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    index_column:
        Column containing frame indices to extract from video blob
    position_column:
        Column containing 2D position of the bounding boxes
    half_size_column:
        Column containing 2D half sizes of the bounding boxes
    blob_column:
        Column containing the video blob
    media_type_column:
        Column containing the media type data for the video blob
    image_buffer_output_column:
        Output column name for the image buffer
    image_format_output_column:
        Output column name for the image format
    perform_cache:
        If True, performs a cache() operation after setting up the UDF.
    remove_nulls:
        If True, performs a filter operation on the resultant DataFrame

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to the components of
    the Image archetype.

    """
    schema = df.schema()
    prior_columns = schema.names
    temp_path = f"{image_buffer_output_column}_struct"

    if isinstance(index_column, str):
        index_column = col(index_column)
    if isinstance(position_column, str):
        position_column = col(position_column)
    if isinstance(half_size_column, str):
        half_size_column = col(half_size_column)
    if isinstance(blob_column, str):
        blob_column = col(blob_column)
    if isinstance(media_type_column, str):
        media_type_column = col(media_type_column)

    extraction_udf = extract_bounding_box_images_from_video_udf()
    df = df.select(
        *prior_columns,
        extraction_udf(index_column, position_column, half_size_column, blob_column, media_type_column).alias(
            temp_path
        ),
    )

    if perform_cache:
        df = df.cache()

    if remove_nulls:
        df = df.filter(col(temp_path).is_not_null())

    return df.select(
        *prior_columns,
        col(temp_path)["ImageBuffer"].alias(image_buffer_output_column),
        col(temp_path)["ImageFormat"].alias(image_format_output_column),
    )


def extract_bounding_box_images_from_video_udf() -> ScalarUDF:
    """Create a UDF to extract bounding box images from a video."""
    return udf(rerun_bindings.datafusion.bounded_image_extraction_udf())


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
