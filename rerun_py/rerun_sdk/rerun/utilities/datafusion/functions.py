"""Rerun DataFusion utility functions."""

from __future__ import annotations

import rerun_bindings
from datafusion import DataFrame, col, udf
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pyarrow as pa

def column_for_component(
        schema: pa.Schema,
        entity_path: str | None,
        archetype_name: str | None,
        component_name: str | None,
) -> str | None:
    for field in schema:
        valid_path = True
        valid_archetype = True
        valid_component = True
        if entity_path is not None:
            valid_path = False
            if b"rerun.entity_path" in field.metadata.keys():
                if field.metadata[b"rerun.entity_path"].decode('utf-8') == entity_path:
                    valid_path = True
        if archetype_name is not None:
            valid_archtype = False
            if b"rerun.archetype" in field.metadata.keys():
                if field.metadata[b"rerun.archetype"].decode('utf-8') == f"rerun.archetypes.{archetype_name}":
                    valid_archtype = True
        if component_name is not None:
            valid_component = False
            if b"rerun.component" in field.metadata.keys():
                if field.metadata[b"rerun.component"].decode('utf-8') == f"rerun.components.{component_name}":
                    valid_component = True
        if valid_path and valid_archetype and valid_component:
            return field.name

    return None

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

def intersection_over_union(
        df: DataFrame,
        entity_1_path: str,
        entity_2_path: str,
        output_path: str,
        remove_nulls: bool = True,
) -> DataFrame:
    """
    Extract images from a video and bounding boxes.

    This is a Rerun focused DataFusion function that will will assume there are two
    entities that contain Boxes2D archetypes. It will identify the corresponding
    data columns and perform a calculation of the area of the intersection of the
    boxes divided by the area of the union of the boxes. For boxes that exactly overlap
    this should yield a scalar value of 1.0 and for boxes that do not intersect it
    will yield a value of 0.0. If either of the columns for the boxes is null, it
    will yield a null.

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

    iou_udf = udf(rerun_bindings.datafusion.intersection_over_union_udf())

    ent1_pos_col = column_for_component(schema, entity_1_path, "Boxes2D", "Position2D")
    ent1_half_size_col = column_for_component(schema, entity_1_path, "Boxes2D", "HalfSize2D")
    ent2_pos_col = column_for_component(schema, entity_2_path, "Boxes2D", "Position2D")
    ent2_half_size_col = column_for_component(schema, entity_2_path, "Boxes2D", "HalfSize2D")

    output_col = f"{output_path}:Scalar"

    df = df.with_column(
        output_col,
        iou_udf(
            col(ent1_pos_col),
            col(ent1_half_size_col),
            col(ent2_pos_col),
            col(ent2_half_size_col),
        )
    )

    if remove_nulls:
        df = df.filter(col(output_col).is_not_null())

    return df.select(
        *prior_columns,
        col(output_col).alias(f"{output_path}:Scalar", metadata={"rerun.entity_path": output_path}),
    )