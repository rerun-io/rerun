from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

import rerun.log.error_utils
from rerun import bindings
from rerun.components.instance import InstanceArray
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

# Fully qualified to avoid circular import

__all__ = [
    "_add_extension_components",
    "log_extension_components",
]

EXT_PREFIX = "ext."

EXT_COMPONENT_TYPES: dict[str, Any] = {}


def _add_extension_components(
    instanced: dict[str, Any],
    splats: dict[str, Any],
    ext: dict[str, Any],
    identifiers: npt.NDArray[np.uint64] | None,
) -> None:
    for name, value in ext.items():
        # Don't log empty components
        if value is None:
            continue

        # Add the ext prefix, unless it's already there
        if not name.startswith(EXT_PREFIX):
            name = EXT_PREFIX + name

        np_type, pa_type = EXT_COMPONENT_TYPES.get(name, (None, None))

        try:
            if np_type is not None:
                np_value = np.atleast_1d(np.array(value, copy=False, dtype=np_type))
                pa_value = pa.array(np_value, type=pa_type)
            else:
                np_value = np.atleast_1d(np.array(value, copy=False))
                pa_value = pa.array(np_value)
                EXT_COMPONENT_TYPES[name] = (np_value.dtype, pa_value.type)
        except Exception as ex:
            rerun.log.error_utils._send_warning(
                "Error converting extension data to arrow for component {}. Dropping.\n{}: {}".format(
                    name, type(ex).__name__, ex
                ),
                1,
            )
            continue

        is_splat = (len(np_value) == 1) and (len(identifiers or []) != 1)

        if is_splat:
            splats[name] = pa_value
        else:
            instanced[name] = pa_value


@log_decorator
def log_extension_components(
    entity_path: str,
    ext: dict[str, Any],
    *,
    identifiers: Sequence[int] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an arbitrary collection of extension components.

    Each item in `ext` will be logged as a separate component.

     - The key will be used as the name of the component
     - The value must be able to be converted to an array of arrow types. In general, if
       you can pass it to [pyarrow.array](https://arrow.apache.org/docs/python/generated/pyarrow.array.html),
       you can log it as a extension component.

    All values must either have the same length, or be singular in which case they will be
    treated as a splat.

    Extension components will be prefixed with "ext." to avoid collisions with rerun native components.
    You do not need to include this prefix; it will be added for you.

    Note: rerun requires that a given component only take on a single type. The first type logged
    will be the type that is used for all future logs of that component. The API will make
    a best effort to do type conversion if supported by numpy and arrow. Any components that
    can't be converted will be dropped.

    If you are want to inspect how your component will be converted to the underlying
    arrow code, the following snippet is what is happening internally:
    ```
    np_value = np.atleast_1d(np.array(value, copy=False))
    pa_value = pa.array(value)
    ```

    Parameters
    ----------
    entity_path:
        Path to the extension components in the space hierarchy.
    ext:
        A dictionary of extension components.
    identifiers:
        Optional identifiers for each component. If provided, must be the same length as the components.
    timeless:
        If true, the components will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    recording = RecordingStream.to_native(recording)

    identifiers_np = np.array((), dtype="uint64")
    if identifiers:
        try:
            identifiers = [int(id) for id in identifiers]
            identifiers_np = np.array(identifiers, dtype="uint64")
        except ValueError:
            rerun.log.error_utils._send_warning("Only integer identifiers supported", 1)

    instanced: dict[str, Any] = {}
    splats: dict[str, Any] = {}

    if len(identifiers_np):
        instanced["rerun.instance_key"] = InstanceArray.from_numpy(identifiers_np)

    _add_extension_components(instanced, splats, ext, identifiers_np)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)
