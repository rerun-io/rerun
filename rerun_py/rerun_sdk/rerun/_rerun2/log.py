from __future__ import annotations

from typing import Any, Iterable, Protocol

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from .. import RecordingStream, bindings
from ..log import error_utils
from . import components as cmp
from ._baseclasses import NamedExtensionArray

__all__ = ["log"]


EXT_PREFIX = "ext."

ext_component_types: dict[str, Any] = {}


class ComponentBatchLike(Protocol):
    """Describes interface for objects that can be converted to rerun Components."""

    def component_name(self) -> str:
        """Returns the name of the component."""
        ...

    def as_arrow_batch(self) -> pa.Array:
        """
        Returns a `pyarrow.Array` of the component data.

        Each element in the array corresponds to an instance of the component. Single-instanced
        components and splats must still be represented as a 1-element array.
        """
        ...


class ArchetypeLike(Protocol):
    """Describes interface for objects that can be logged via rr.log."""

    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        """
        Returns an iterable of `ComponentBatchLike` objects.

        Each object in the iterable must adhere to the `ComponentBatchLike`
        interface. All of the batches should have the same length as the value
        returned by `num_instances`, or length 1 if the component is a splat.,
        or 0 if the component is being cleared.
        """
        ...

    def num_instances(self) -> int:
        """
        The number of instances in each batch.

        Each batch returned by `as_component_batches` should have this number of
        elements, or 1 in the case it is a splat, or 0 in the case that
        component is being cleared.
        """
        ...


# adapted from rerun.log._add_extension_components
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
            error_utils._send_warning(
                f"Error converting extension data to arrow for component {name}. Dropping.\n{type(ex).__name__}: {ex}",
                1,
            )
            continue

        is_splat = (len(np_value) == 1) and (len(identifiers or []) != 1)

        if is_splat:
            splats[name] = pa_value  # noqa
        else:
            instanced[name] = pa_value  # noqa


def _splat() -> cmp.InstanceKeyArray:
    """Helper to generate a splat InstanceKeyArray."""

    _MAX_U64 = 2**64 - 1
    return pa.array([_MAX_U64], type=cmp.InstanceKeyType().storage_type)  # type: ignore[no-any-return]


def log(
    entity_path: str,
    entity: ArchetypeLike,
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
    entity: Archetype
        The archetype object representing the entity.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the entity will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    instanced: dict[str, NamedExtensionArray] = {}
    splats: dict[str, NamedExtensionArray] = {}

    num_instances = entity.num_instances()
    components = list(entity.as_component_batches())
    names = [comp.component_name() for comp in components]
    arrow_arrays = [comp.as_arrow_batch() for comp in components]

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

    # Always the instanced components last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(  # pyright: ignore[reportGeneralTypeIssues]
        entity_path,
        components=instanced,
        timeless=timeless,
        recording=recording,
    )
