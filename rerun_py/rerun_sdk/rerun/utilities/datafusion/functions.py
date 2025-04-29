"""Rerun DataFusion utility functions."""

from __future__ import annotations

import rerun_bindings
from datafusion import DataFrame, col, udf


def duplicate_components(df: DataFrame, input_path: str, output_path: str, components: list[str]) -> DataFrame:
    """
    Duplicate components from one entity path to another.

    In addition to duplicating the data from one column to another, this sets
    the appropriate Rerun metadata for the entity path on the output.

    This is a simple function that does not perform checking that the component
    names are valid Rerun components. During planning it will verify the input
    paths exist.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    input_path:
        Entity path to copy components from
    output_path:
        Entity path to copy components to
    components:
        List of component names to copy.

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to the components of
    the Points3D archetype.

    """
    for component in components:
        df = df.with_column(
            f"{output_path}:{component}",
            col(f"{input_path}:{component}").alias(
                f"{output_path}:{component}", metadata={"rerun.entity_path": output_path}
            ),
        )

    return df


def convert_depth_image_to_point_cloud(
    df: DataFrame, depth_image_path: str, pinhole_path: str, output_path: str, remove_nulls: bool = True
) -> DataFrame:
    """
    Convert a depth image to a point cloud.

    This is a Rerun DataFusion function that will convert from a DepthImage and Pinhole
    into a Points3D.

    There is an assumption with this UDF that the column names of the DataFrame correspond
    to the Rerun data model. Each column will be the full entity path followed by the
    component name. For example, if you have an entity called `/world/my_robot` that
    contains your AssetVideo, we will expect the MediaType component to have column name
    `/world/my_robot:MediaType`.

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
    perform_cache:
        If True, performs a cache() operation after setting up the UDF.
    remove_nulls:
        If True, performs a filter operation on the resultant DataFrame

    Returns
    -------
    A DataFusion DataFrame with new columns corresponding to the components of
    the Points3D archetype.

    """
    prior_columns = df.schema().names
    temp_path = f"{output_path}_udf"
    depth_to_cloud = udf(rerun_bindings.datafusion.depth_image_to_point_cloud_udf())

    df = df.select(
        *prior_columns,
        depth_to_cloud(
            col(f"{depth_image_path}:ImageBuffer"),
            col(f"{depth_image_path}:ImageFormat"),
            col(f"{depth_image_path}:DepthMeter"),
            col(f"{pinhole_path}:PinholeProjection"),
            col(f"{pinhole_path}:Resolution"),
        ).alias(temp_path),
    )

    if remove_nulls:
        df = df.filter(col(temp_path)["Position3D"].is_not_null())

    return df.select(
        *prior_columns,
        col(temp_path)["Points3DIndicator"].alias(
            f"{output_path}:Points3DIndicator", metadata={"rerun.entity_path": output_path}
        ),
        col(temp_path)["Position3D"].alias(f"{output_path}:Position3D", metadata={"rerun.entity_path": output_path}),
    )


def extract_bounding_box_images_from_video(
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

    There is an assumption with this UDF that the column names of the DataFrame correspond
    to the Rerun data model. Each column will be the full entity path followed by the
    component name. For example, if you have an entity called `/world/my_robot` that
    contains your AssetVideo, we will expect the MediaType component to have column name
    `/world/my_robot:MediaType`.

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
    prior_columns = df.schema().names
    temp_path = f"{output_path}_extracted_images"
    bounded_image_extraction = udf(rerun_bindings.datafusion.bounded_image_extraction_udf())

    df = df.select(
        *prior_columns,
        bounded_image_extraction(
            col(frame_column),
            col(f"{boxes_2d_path}:Position2D"),
            col(f"{boxes_2d_path}:HalfSize2D"),
            col(f"{asset_video_path}:Blob"),
            col(f"{asset_video_path}:MediaType"),
        ).alias(temp_path),
    )

    if perform_cache:
        df = df.cache()

    if remove_nulls:
        df = df.filter(col(temp_path).is_not_null())

    return df.select(
        *prior_columns,
        col(temp_path)["ImageBuffer"].alias(f"{output_path}:ImageBuffer", metadata={"rerun.entity_path": output_path}),
        col(temp_path)["ImageFormat"].alias(f"{output_path}:ImageFormat", metadata={"rerun.entity_path": output_path}),
    )
