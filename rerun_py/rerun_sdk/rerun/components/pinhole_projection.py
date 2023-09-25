# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/pinhole_projection.fbs".

# You can extend this class by creating a "PinholeProjectionExt" class in "pinhole_projection_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["PinholeProjection", "PinholeProjectionBatch", "PinholeProjectionType"]


class PinholeProjection(datatypes.Mat3x3):
    """
    Camera projection, from image coordinates to view coordinates.

    Child from parent.
    Image coordinates from camera view coordinates.

    Example:
    -------
    ```text
    1496.1     0.0  980.5
       0.0  1496.1  744.5
       0.0     0.0    1.0
    ```
    """

    # You can define your own __init__ function as a member of PinholeProjectionExt in pinhole_projection_ext.py

    # Note: there are no fields here because PinholeProjection delegates to datatypes.Mat3x3
    pass


class PinholeProjectionType(datatypes.Mat3x3Type):
    _TYPE_NAME: str = "rerun.components.PinholeProjection"


class PinholeProjectionBatch(datatypes.Mat3x3Batch, ComponentBatchMixin):
    _ARROW_TYPE = PinholeProjectionType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(PinholeProjectionType())
