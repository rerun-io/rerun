# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/pinhole.fbs".

# You can extend this class by creating a "PinholeExt" class in "pinhole_ext.py".

from __future__ import annotations

import numpy as np
import pyarrow as pa
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions
from .pinhole_ext import PinholeExt

__all__ = ["Pinhole"]


@define(str=False, repr=False, init=False)
class Pinhole(PinholeExt, Archetype):
    """
    **Archetype**: Camera perspective projection (a.k.a. intrinsics).

    ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**

    Examples
    --------
    ### Simple pinhole camera:
    ```python
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_pinhole", spawn=True)
    rng = np.random.default_rng(12345)

    image = rng.uniform(0, 255, size=[3, 3, 3])
    rr.log("world/image", rr.Pinhole(focal_length=3, width=3, height=3))
    rr.log("world/image", rr.Image(image))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/1200w.png">
      <img src="https://static.rerun.io/pinhole_simple/9af9441a94bcd9fd54e1fea44fb0c59ff381a7f2/full.png" width="640">
    </picture>
    </center>

    ### Perspective pinhole camera:
    ```python
    import rerun as rr

    rr.init("rerun_example_pinhole_perspective", spawn=True)

    rr.log(
        "world/cam",
        rr.Pinhole(fov_y=0.7853982, aspect_ratio=1.7777778, camera_xyz=rr.ViewCoordinates.RUB, image_plane_distance=0.1),
    )

    rr.log("world/points", rr.Points3D([(0.0, 0.0, -0.5), (0.1, 0.1, -0.5), (-0.1, -0.1, -0.5)], radii=0.025))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/1200w.png">
      <img src="https://static.rerun.io/pinhole_perspective/317e2de6d212b238dcdad5b67037e9e2a2afafa0/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in pinhole_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            image_from_camera=None,
            resolution=None,
            camera_xyz=None,
            image_plane_distance=None,
        )

    @classmethod
    def _clear(cls) -> Pinhole:
        """Produce an empty Pinhole, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        image_from_camera: datatypes.Mat3x3Like | None = None,
        resolution: datatypes.Vec2DLike | None = None,
        camera_xyz: datatypes.ViewCoordinatesLike | None = None,
        image_plane_distance: datatypes.Float32Like | None = None,
    ) -> Pinhole:
        """
        Update only some specific fields of a `Pinhole`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        image_from_camera:
            Camera projection, from image coordinates to view coordinates.
        resolution:
            Pixel resolution (usually integers) of child image space. Width and height.

            Example:
            ```text
            [1920.0, 1440.0]
            ```

            `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        camera_xyz:
            Sets the view coordinates for the camera.

            All common values are available as constants on the [`components.ViewCoordinates`][rerun.components.ViewCoordinates] class.

            The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is also the recommended setting.
            This means that the camera frustum will point along the positive Z axis of the parent space,
            and the cameras "up" direction will be along the negative Y axis of the parent space.

            The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
            When logging a depth image under this entity, this is the direction the point cloud will be projected.
            With `RDF`, the default forward is +Z.

            The frustum's "up" direction will be whichever axis is set to `U` (or the opposite of `D`).
            This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
            With `RDF`, the default is up is -Y.

            The frustum's "right" direction will be whichever axis is set to `R` (or the opposite of `L`).
            This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
            With `RDF`, the default right is +x.

            Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).

            NOTE: setting this to something else than `RDF` (the default) will change the orientation of the camera frustum,
            and make the pinhole matrix not match up with the coordinate system of the pinhole entity.

            The pinhole matrix (the `image_from_camera` argument) always project along the third (Z) axis,
            but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
        image_plane_distance:
            The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.

            This is only used for visualization purposes, and does not affect the projection itself.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "image_from_camera": image_from_camera,
                "resolution": resolution,
                "camera_xyz": camera_xyz,
                "image_plane_distance": image_plane_distance,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> Pinhole:
        """Clear all the fields of a `Pinhole`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        image_from_camera: datatypes.Mat3x3ArrayLike | None = None,
        resolution: datatypes.Vec2DArrayLike | None = None,
        camera_xyz: datatypes.ViewCoordinatesArrayLike | None = None,
        image_plane_distance: datatypes.Float32ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        image_from_camera:
            Camera projection, from image coordinates to view coordinates.
        resolution:
            Pixel resolution (usually integers) of child image space. Width and height.

            Example:
            ```text
            [1920.0, 1440.0]
            ```

            `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
        camera_xyz:
            Sets the view coordinates for the camera.

            All common values are available as constants on the [`components.ViewCoordinates`][rerun.components.ViewCoordinates] class.

            The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is also the recommended setting.
            This means that the camera frustum will point along the positive Z axis of the parent space,
            and the cameras "up" direction will be along the negative Y axis of the parent space.

            The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
            When logging a depth image under this entity, this is the direction the point cloud will be projected.
            With `RDF`, the default forward is +Z.

            The frustum's "up" direction will be whichever axis is set to `U` (or the opposite of `D`).
            This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
            With `RDF`, the default is up is -Y.

            The frustum's "right" direction will be whichever axis is set to `R` (or the opposite of `L`).
            This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
            With `RDF`, the default right is +x.

            Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).

            NOTE: setting this to something else than `RDF` (the default) will change the orientation of the camera frustum,
            and make the pinhole matrix not match up with the coordinate system of the pinhole entity.

            The pinhole matrix (the `image_from_camera` argument) always project along the third (Z) axis,
            but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
        image_plane_distance:
            The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.

            This is only used for visualization purposes, and does not affect the projection itself.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                image_from_camera=image_from_camera,
                resolution=resolution,
                camera_xyz=camera_xyz,
                image_plane_distance=image_plane_distance,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "image_from_camera": image_from_camera,
            "resolution": resolution,
            "camera_xyz": camera_xyz,
            "image_plane_distance": image_plane_distance,
        }
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]

                if pa.types.is_fixed_size_list(arrow_array.type) and len(shape) <= 2:
                    # If shape length is 2 or less, we have `num_rows` single element batches (each element is a fixed sized list).
                    # `shape[1]` should be the length of the fixed sized list.
                    # (This should have been already validated by conversion to the arrow_array)
                    batch_length = 1
                else:
                    batch_length = shape[1] if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    image_from_camera: components.PinholeProjectionBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PinholeProjectionBatch._converter,  # type: ignore[misc]
    )
    # Camera projection, from image coordinates to view coordinates.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    resolution: components.ResolutionBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ResolutionBatch._converter,  # type: ignore[misc]
    )
    # Pixel resolution (usually integers) of child image space. Width and height.
    #
    # Example:
    # ```text
    # [1920.0, 1440.0]
    # ```
    #
    # `image_from_camera` project onto the space spanned by `(0,0)` and `resolution - 1`.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    camera_xyz: components.ViewCoordinatesBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ViewCoordinatesBatch._converter,  # type: ignore[misc]
    )
    # Sets the view coordinates for the camera.
    #
    # All common values are available as constants on the [`components.ViewCoordinates`][rerun.components.ViewCoordinates] class.
    #
    # The default is `ViewCoordinates::RDF`, i.e. X=Right, Y=Down, Z=Forward, and this is also the recommended setting.
    # This means that the camera frustum will point along the positive Z axis of the parent space,
    # and the cameras "up" direction will be along the negative Y axis of the parent space.
    #
    # The camera frustum will point whichever axis is set to `F` (or the opposite of `B`).
    # When logging a depth image under this entity, this is the direction the point cloud will be projected.
    # With `RDF`, the default forward is +Z.
    #
    # The frustum's "up" direction will be whichever axis is set to `U` (or the opposite of `D`).
    # This will match the negative Y direction of pixel space (all images are assumed to have xyz=RDF).
    # With `RDF`, the default is up is -Y.
    #
    # The frustum's "right" direction will be whichever axis is set to `R` (or the opposite of `L`).
    # This will match the positive X direction of pixel space (all images are assumed to have xyz=RDF).
    # With `RDF`, the default right is +x.
    #
    # Other common formats are `RUB` (X=Right, Y=Up, Z=Back) and `FLU` (X=Forward, Y=Left, Z=Up).
    #
    # NOTE: setting this to something else than `RDF` (the default) will change the orientation of the camera frustum,
    # and make the pinhole matrix not match up with the coordinate system of the pinhole entity.
    #
    # The pinhole matrix (the `image_from_camera` argument) always project along the third (Z) axis,
    # but will be re-oriented to project along the forward axis of the `camera_xyz` argument.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    image_plane_distance: components.ImagePlaneDistanceBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ImagePlaneDistanceBatch._converter,  # type: ignore[misc]
    )
    # The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
    #
    # This is only used for visualization purposes, and does not affect the projection itself.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
