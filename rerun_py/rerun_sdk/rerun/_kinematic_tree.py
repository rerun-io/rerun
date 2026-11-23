"""The parts of `rerun.urdf` and `rerun.mjcf` that do not depend on the model format."""

from __future__ import annotations

import warnings
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Sequence

    import pyarrow as pa

    from rerun_bindings import _MjcfJointInternal, _MjcfTreeInternal, _UrdfJointInternal, _UrdfTreeInternal

    from . import Transform3D
    from ._baseclasses import ComponentColumnList
    from .chunk import LazyChunkStream
    from .recording_stream import RecordingStream


class _KinematicJoint:
    def __init__(self, inner: _UrdfJointInternal | _MjcfJointInternal) -> None:
        self._inner = inner

    def compute_transform(self, value: float, clamp: bool = True) -> Transform3D:
        """
        Compute a Transform3D for this joint at the given value.

        Parameters
        ----------
        value:
            Joint value: an angle in radians for rotating joints, or a distance in meters for sliding joints.
            Values outside limits are clamped with a warning if `clamp` is True.
        clamp:
            Whether to clamp & warn about values outside joint limits.

        Returns
        -------
        Transform3D
            Transform with rotation, translation, parent_frame, and child_frame ready to log.

        """
        from . import Transform3D
        from .encodings import Quaternion

        result = self._inner.compute_transform(value, clamp=clamp)

        for warning in result["warnings"]:
            warnings.warn(warning, UserWarning, stacklevel=2)

        return Transform3D(
            quaternion=Quaternion(xyzw=result["quaternion_xyzw"]),
            translation=result["translation"],
            parent_frame=result["parent_frame"],
            child_frame=result["child_frame"],
        )

    def compute_transform_columns(self, values: Sequence[float], *, clamp: bool = True) -> ComponentColumnList:
        """
        Compute transforms for this joint at multiple values, returning columnar data for use with `send_columns`.

        Parameters
        ----------
        values:
            Joint values: angles in radians for rotating joints, or distances in meters for sliding joints.
            Values outside limits are clamped with a warning if `clamp` is True.
        clamp:
            Whether to clamp & warn about values outside joint limits.

        Returns
        -------
        ComponentColumnList
            Columnar transform data ready for use with [`rerun.send_columns`][].

        """
        from . import Transform3D

        result = self._inner.compute_transform_columns(list(values), clamp=clamp)

        for warning in result["warnings"]:
            warnings.warn(warning, UserWarning, stacklevel=2)

        return Transform3D.columns(
            translation=result["translations"],
            quaternion=result["quaternions_xyzw"],
            parent_frame=[result["parent_frame"]] * len(values),
            child_frame=[result["child_frame"]] * len(values),
        )

    def __repr__(self) -> str:
        return self._inner.__repr__()


class _KinematicTree:
    def __init__(self, inner: _UrdfTreeInternal | _MjcfTreeInternal) -> None:
        self._inner = inner

    @property
    def name(self) -> str:
        """Name of the model."""
        return self._inner.name

    @property
    def frame_prefix(self) -> str | None:
        """The frame prefix, if set."""
        return self._inner.frame_prefix

    def log_to_recording(self, recording: RecordingStream | None = None) -> None:
        """
        Log the full model (geometry + static transforms) to a recording stream.

        This can be used as alternative to [`rerun.log_file_from_path`][],
        especially in cases where you need the extra configuration options of loading the model yourself
        (e.g. `frame_prefix` for multi-robot setups).

        Parameters
        ----------
        recording:
            The recording stream to log to. If `None`, the current active recording is used.

        """
        self._inner.log(recording.to_native() if recording is not None else None)

    def stream(self, *, include_joint_transforms: bool = True) -> LazyChunkStream:
        """
        Return a lazy stream over chunks emitted from this model.

        Parameters
        ----------
        include_joint_transforms:
            Whether to include the static joint transforms of the model's reference configuration.

        """
        from .chunk import LazyChunkStream

        return LazyChunkStream(self._inner.stream(include_joint_transforms=include_joint_transforms))

    def compute_joint_transform_batches(
        self,
        names: pa.Array,
        values: pa.Array,
        *,
        clamp: bool = False,
    ) -> pa.Array:
        """
        Compute batches of 3D transform components from Arrow list arrays containing joint names and values.

        `names` must be a `ListArray` with `Utf8` values.
        `values` must be a `ListArray` with values castable to `Float64`.

        The output is a `ListArray` with `translation`, `quaternion`, `parent_frame`, and
        `child_frame` fields and the same outer row count as the inputs.
        Each output row has one transform per named joint, in order.

        Note: this is intended as a helper for lens pipelines, where you would usually pipe this output
        through an additional lens that scatters each batch into final `Transform3D` component rows.

        Parameters
        ----------
        names:
            Joint names for each row.
        values:
            Joint values for each row.
        clamp:
            Whether to clamp & warn about values outside joint limits.

        Returns
        -------
        pa.Array
            Transform batches with one outer row for each input row.

        """
        return self._inner.compute_joint_transform_batches(names, values, clamp=clamp)

    def __repr__(self) -> str:
        return self._inner.__repr__()
