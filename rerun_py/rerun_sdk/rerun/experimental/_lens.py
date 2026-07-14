from __future__ import annotations

from typing import TYPE_CHECKING, Literal, TypeAlias

from rerun._baseclasses import ComponentDescriptor
from rerun_bindings import DeriveLensInternal, MutateLensInternal

from ._selector import Selector

if TYPE_CHECKING:
    import pyarrow as pa


class DeriveLens:
    """
    A derive lens that creates new component/time columns from an input component.

    Derive lenses extract fields from a component and produce new columns,
    optionally at a different entity and/or with new time columns.

    Pass `scatter=True` to enable 1:N row mapping (exploding lists).

    Example usage::

        lens = (
            DeriveLens("Imu:accel")
            .to_component(rr.Scalars.descriptor_scalars(), Selector(".x"))
        )

    To write to an explicit target entity::

        lens = (
            DeriveLens("Imu:accel", output_entity="/out/x")
            .to_component(rr.Scalars.descriptor_scalars(), Selector(".x"))
        )

    """

    _internal: DeriveLensInternal

    def __init__(
        self,
        input_component: str,
        *,
        output_entity: str | None = None,
        scatter: bool = False,
    ) -> None:
        """
        Create a new derive lens.

        Parameters
        ----------
        input_component:
            The component identifier to match (e.g. `"Imu:accel"`).
        output_entity:
            Optional target entity path. When set, output is written
            to this entity instead of the input entity.
        scatter:
            When `True`, use 1:N row mapping (explode lists).

        """
        self._internal = DeriveLensInternal(
            input_component,
            output_entity=output_entity,
            scatter=scatter,
        )

    def to_component(
        self,
        component: ComponentDescriptor | str,
        selector: Selector | str,
        *,
        cast_to: pa.DataType | Literal["auto"] | None = None,
    ) -> DeriveLens:
        """
        Add a component output column.

        Parameters
        ----------
        component:
            A `ComponentDescriptor` or a component identifier string
            for the output column (e.g. `"Scalars:scalars"`).
        selector:
            A [`Selector`][rerun.experimental.Selector] or selector query string to apply to the
            input column.
        cast_to:
            How to cast the produced column to match the target component. By default
            (`None`) the column is emitted as-is. Pass `"auto"` to cast it to the
            component's canonical Arrow datatype, or an explicit pyarrow `DataType` to
            cast it to that type. Casting errors if the conversion is unsupported.

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the component added.

        """
        sel = _normalize_selector(selector)
        new = DeriveLens.__new__(DeriveLens)
        if isinstance(component, str):
            component = ComponentDescriptor(component)
        new._internal = self._internal.to_component(component, sel._internal, cast_to=cast_to)
        return new

    def to_timeline(
        self,
        timeline_name: str,
        timeline_type: Literal["sequence", "duration_ns", "timestamp_ns"],
        selector: Selector | str,
    ) -> DeriveLens:
        """
        Add a time extraction column.

        Parameters
        ----------
        timeline_name:
            Name of the timeline to create.
        timeline_type:
            Type of the timeline: `"sequence"`, `"duration_ns"`,
            or `"timestamp_ns"`.
        selector:
            A [`Selector`][rerun.experimental.Selector] or selector query string to extract time
            values (must produce `Int64` arrays).

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the time column added.

        """
        sel = _normalize_selector(selector)
        new = DeriveLens.__new__(DeriveLens)
        new._internal = self._internal.to_timeline(timeline_name, timeline_type, sel._internal)
        return new

    def to_packed_component(
        self, component: ComponentDescriptor | str, *fields: str, cast_to: pa.DataType | Literal["auto"] | None = "auto"
    ) -> DeriveLens:
        """
        Add a component output column by packing the provided fields in a fixed-size list.

        Parameters
        ----------
        component:
            A `ComponentDescriptor` or a component identifier string for the output column
            (e.g. `"Points3D:positions"`).
        *fields:
            Names of the struct fields to pack, in order. They must all resolve to the same
            datatype. At least one field is required.
        cast_to:
            How to cast the packed column to match the target component. Defaults to `"auto"`,
            which casts to the component's canonical Arrow datatype (e.g. the `f64` columns
            a parquet file typically holds → the `f32` a `Transform3D:translation` expects).
            Pass an explicit pyarrow `DataType` to cast to that type, or `None` to emit the
            packed list as-is.


        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the packed component added.

        """
        if not fields:
            raise ValueError("to_packed_component requires at least one field")
        selector = f"pack({', '.join(f'.{field}!' for field in fields)})"
        return self.to_component(component, selector, cast_to=cast_to)

    # TODO(RR-5007): this should ideally be codegened
    def to_translation(self, x: str, y: str, z: str) -> DeriveLens:
        """
        Add a `Transform3D:translation` component from the provided paths.

        Parameters
        ----------
        x, y, z:
            Paths of the struct fields holding the translation components.

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the translation added.

        """
        from rerun.archetypes import Transform3D

        return self.to_packed_component(Transform3D.descriptor_translation(), x, y, z, cast_to="auto")

    # TODO(RR-5007): this should ideally be codegened
    def to_quaternion(self, x: str, y: str, z: str, w: str) -> DeriveLens:  # noqa: PLR0917
        """
        Add a `Transform3D:quaternion` component from the provided paths.

        Parameters
        ----------
        x, y, z, w:
            Paths of the struct fields holding the quaternion components, in `xyzw` order.

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the quaternion added.

        """
        from rerun.archetypes import Transform3D

        return self.to_packed_component(Transform3D.descriptor_quaternion(), x, y, z, w, cast_to="auto")

    # TODO(RR-5007): this should ideally be codegened
    def to_scale(self, x: str, y: str, z: str) -> DeriveLens:
        """
        Add a `Transform3D:scale` component from the provided paths.

        Parameters
        ----------
        x, y, z:
            Paths of the struct fields holding the per-axis scale factors.

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the scale added.

        """
        from rerun.archetypes import Transform3D

        return self.to_packed_component(Transform3D.descriptor_scale(), x, y, z, cast_to="auto")

    # TODO(RR-5007): this should ideally be codegened
    def to_rotation_axis_angle(self, axis_x: str, axis_y: str, axis_z: str, angle: str) -> DeriveLens:  # noqa: PLR0917
        """
        Add a `Transform3D:rotation_axis_angle` component from the provided paths.

        Parameters
        ----------
        axis_x, axis_y, axis_z:
            Paths of the struct fields holding the rotation axis components.
        angle:
            Path of the struct field holding the rotation angle, in radians.

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the rotation added.

        """
        from rerun.archetypes import Transform3D

        # TODO(RR-4999): drop the .pipe and use a struct-constructing selector once structs are supported
        selector = Selector(".").pipe(
            lambda struct: _build_rotation_axis_angle_struct(struct, axis_x, axis_y, axis_z, angle)
        )
        return self.to_component(Transform3D.descriptor_rotation_axis_angle(), selector)

    # TODO(RR-5007): this should ideally be codegened
    def to_scalars(self, *fields: str) -> DeriveLens:
        """
        Add a `Scalars:scalars` component from the provided path(s).

        Each path becomes one scalar instance per row, so a single path yields one series and
        multiple paths yield one series each at the same entity.

        Parameters
        ----------
        *fields:
            Paths of the struct fields to read as scalars, in order. At least one is required.

        Returns
        -------
        A new [`DeriveLens`][rerun.experimental.DeriveLens] with the scalars added.

        """
        from rerun.archetypes import Scalars

        if not fields:
            raise ValueError("to_scalars requires at least one field")
        if len(fields) == 1:
            # A single scalar must stay a plain value, not a 1-element fixed-size list.
            return self.to_component(Scalars.descriptor_scalars(), f".{fields[0]}", cast_to=None)
        # Pack into a fixed-size list, then flatten with `.[]` so the values land as N
        # instances in the per-row list (`List<T>`) rather than a nested fixed-size list.
        packed = ", ".join(f".{field}!" for field in fields)
        return self.to_component(Scalars.descriptor_scalars(), f"pack({packed}) | .[]", cast_to=None)


class MutateLens:
    """
    A mutate lens that modifies the input component in-place.

    Mutate lenses apply a selector transformation to the input component,
    replacing it in the chunk. By default, new row IDs are generated.
    Pass `keep_row_ids=True` to preserve original row IDs.

    Example usage::

        lens = MutateLens("Imu:accel", Selector(".x"))

    """

    _internal: MutateLensInternal

    def __init__(
        self,
        input_component: str,
        selector: Selector | str,
        *,
        keep_row_ids: bool = False,
    ) -> None:
        """
        Create a new mutate lens.

        Parameters
        ----------
        input_component:
            The component identifier to modify in-place.
        selector:
            A [`Selector`][rerun.experimental.Selector] or selector query string to apply.
        keep_row_ids:
            When `True`, preserve the original row IDs.

        """
        sel = _normalize_selector(selector)
        self._internal = MutateLensInternal(
            input_component,
            sel._internal,
            keep_row_ids=keep_row_ids,
        )


Lens: TypeAlias = DeriveLens | MutateLens
"""Union of all lens types."""


def _normalize_selector(selector: Selector | str) -> Selector:
    """Normalize a selector argument to a Selector object."""
    if isinstance(selector, str):
        return Selector(selector)
    return selector


def _interleave_to_fsl(arrays: list[pa.Array], dtype: pa.DataType) -> pa.FixedSizeListArray:
    """Interleave same-length arrays row-wise into a `FixedSizeList(len(arrays), dtype)` with non-null items."""
    import numpy as np
    import pyarrow as pa
    import pyarrow.compute as pc

    columns = [pc.cast(array, dtype).to_numpy(zero_copy_only=False) for array in arrays]
    flat = pa.array(np.stack(columns, axis=1).reshape(-1), type=dtype)
    return pa.FixedSizeListArray.from_arrays(flat, type=pa.list_(pa.field("item", dtype, nullable=False), len(arrays)))


def _build_rotation_axis_angle_struct(  # noqa: PLR0917
    struct: pa.StructArray,
    axis_x: str,
    axis_y: str,
    axis_z: str,
    angle: str,
) -> pa.StructArray:
    """Build the exact `Struct{axis: FixedSizeList<f32>[3], angle: f32}` a `RotationAxisAngle` expects."""
    import pyarrow as pa
    import pyarrow.compute as pc

    axis = _interleave_to_fsl([struct.field(axis_x), struct.field(axis_y), struct.field(axis_z)], pa.float32())
    angle_arr = pc.cast(struct.field(angle), pa.float32())
    return pa.StructArray.from_arrays(
        [axis, angle_arr],
        fields=[
            pa.field("axis", axis.type, nullable=False),
            pa.field("angle", pa.float32(), nullable=False),
        ],
    )
