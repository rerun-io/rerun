from __future__ import annotations

from typing import Any, Iterable

import numpy as np
import numpy.typing as npt
import pyarrow as pa
import rerun_bindings as bindings

from . import components as cmp
from ._baseclasses import AsComponents, ComponentBatchLike
from .error_utils import _send_warning
from .recording_stream import RecordingStream

__all__ = ["log", "IndicatorComponentBatch", "AsComponents"]


EXT_PREFIX = "ext."

ext_component_types: dict[str, Any] = {}


class IndicatorComponentBatch:
    """
    A batch of Indicator Components that can be included in a Bundle.

    Indicator Components signal that a given Bundle should prefer to be interpreted as a
    given archetype. This helps the view heuristics choose the correct view in situations
    where multiple archetypes would otherwise be overlapping.

    This implements the `ComponentBatchLike` interface.
    """

    data: pa.Array

    def __init__(self, archetype_name: str) -> None:
        """
        Creates a new indicator component based on a given `archetype_name`.

        Parameters
        ----------
        archetype_name:
            The fully qualified name of the Archetype.
        """
        self.data = pa.nulls(1, type=pa.null())
        self._archetype_name = archetype_name

    def component_name(self) -> str:
        return self._archetype_name.replace("archetypes", "components") + "Indicator"

    def as_arrow_array(self) -> pa.Array:
        return self.data


# adapted from rerun.log_deprecated._add_extension_components
def _add_extension_components(
    instanced: dict[str, pa.ExtensionArray],
    splats: dict[str, pa.ExtensionArray],
    ext: dict[str, Any],
    identifiers: npt.NDArray[np.uint64] | None,
) -> None:
    global ext_component_types

    for name, value in ext.items():
        # Don't log empty components
        if value is None:
            continue

        # Add the ext prefix, unless it's already there
        if not name.startswith(EXT_PREFIX):
            name = EXT_PREFIX + name

        np_type, pa_type = ext_component_types.get(name, (None, None))

        try:
            if np_type is not None:
                np_value = np.atleast_1d(np.array(value, copy=False, dtype=np_type))
                pa_value = pa.array(np_value, type=pa_type)
            else:
                np_value = np.atleast_1d(np.array(value, copy=False))
                pa_value = pa.array(np_value)
                ext_component_types[name] = (np_value.dtype, pa_value.type)
        except Exception as ex:
            _send_warning(
                f"Error converting extension data to arrow for component {name}. Dropping.\n{type(ex).__name__}: {ex}",
                1,
            )
            continue

        is_splat = (len(np_value) == 1) and (len(identifiers or []) != 1)

        if is_splat:
            splats[name] = pa_value  # noqa
        else:
            instanced[name] = pa_value  # noqa


def _splat() -> cmp.InstanceKeyBatch:
    """Helper to generate a splat InstanceKeyArray."""

    _MAX_U64 = 2**64 - 1
    return pa.array([_MAX_U64], type=cmp.InstanceKeyType().storage_type)  # type: ignore[no-any-return]


def log(
    entity_path: str,
    entity: AsComponents | Iterable[ComponentBatchLike],
    *,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an entity.

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.
    entity:
        Anything that can be converted into a rerun Archetype.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the entity will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    # TODO(jleibs): Profile is_instance with runtime_checkable vs has_attr
    # Note from: https://docs.python.org/3/library/typing.html#typing.runtime_checkable
    #
    # An isinstance() check against a runtime-checkable protocol can be
    # surprisingly slow compared to an isinstance() check against a non-protocol
    # class. Consider using alternative idioms such as hasattr() calls for
    # structural checks in performance-sensitive code. hasattr is
    if hasattr(entity, "as_component_batches"):
        components = entity.as_component_batches()
    else:
        components = entity

    if hasattr(entity, "num_instances"):
        num_instances = entity.num_instances()
    else:
        num_instances = None

    log_components(
        entity_path=entity_path,
        components=components,
        num_instances=num_instances,
        ext=ext,
        timeless=timeless,
        recording=recording,
    )


def log_components(
    entity_path: str,
    components: Iterable[ComponentBatchLike],
    *,
    num_instances: int | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an entity from a collection of `ComponentBatchLike` objects.

    All of the batches should have the same length as the value of
    `num_instances`, or length 1 if the component is a splat., or 0 if the
    component is being cleared.

    Parameters
    ----------
    entity_path:
        Path to the entity in the space hierarchy.
    components:
        A collection of `ComponentBatchLike` objects that
    num_instances:
        Optional. The number of instances in each batch. If not provided, the max of all
        components will be used instead.
    ext:
        Optional dictionary of extension components. See
        [rerun.log_extension_components][]
    timeless:
        If true, the entity will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use. If left unspecified,
        defaults to the current active data recording, if there is one. See
        also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    instanced: dict[str, pa.Array] = {}
    splats: dict[str, pa.Array] = {}

    components = list(components)

    names = [comp.component_name() for comp in components]
    arrow_arrays = [comp.as_arrow_array() for comp in components]

    if num_instances is None:
        num_instances = max(len(arr) for arr in arrow_arrays)

    for name, array in zip(names, arrow_arrays):
        # Strip off the ExtensionArray if it's present. We will always log via component_name.
        # TODO(jleibs): Maybe warn if there is a name mismatch here.
        if isinstance(array, pa.ExtensionArray):
            array = array.storage

        if len(array) == 1 and num_instances > 1:
            splats[name] = array
        else:
            instanced[name] = array

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.components.InstanceKey"] = _splat()
        bindings.log_arrow_msg(  # pyright: ignore[reportGeneralTypeIssues]
            entity_path,
            components=splats,
            timeless=timeless,
            recording=recording,
        )

    # Always log the instanced components last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(  # pyright: ignore[reportGeneralTypeIssues]
        entity_path,
        components=instanced,
        timeless=timeless,
        recording=recording,
    )
