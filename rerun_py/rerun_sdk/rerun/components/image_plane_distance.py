# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/image_plane_distance.fbs".

# You can extend this class by creating a "ImagePlaneDistanceExt" class in "image_plane_distance_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["ImagePlaneDistance", "ImagePlaneDistanceBatch", "ImagePlaneDistanceType"]


class ImagePlaneDistance(datatypes.Float32):
    """
    **Component**: The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.

    This is only used for visualization purposes, and does not affect the projection itself.
    """

    # You can define your own __init__ function as a member of ImagePlaneDistanceExt in image_plane_distance_ext.py

    # Note: there are no fields here because ImagePlaneDistance delegates to datatypes.Float32
    pass


class ImagePlaneDistanceType(datatypes.Float32Type):
    _TYPE_NAME: str = "rerun.components.ImagePlaneDistance"


class ImagePlaneDistanceBatch(datatypes.Float32Batch, ComponentBatchMixin):
    _ARROW_TYPE = ImagePlaneDistanceType()
