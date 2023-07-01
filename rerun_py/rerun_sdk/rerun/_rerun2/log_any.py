from __future__ import annotations

from dataclasses import fields
from typing import Any

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from .. import RecordingStream, bindings
from ..log import error_utils
from .archetypes import Archetype
from .components import Component, InstanceKeyArray

__all__ = ["log_any"]


EXT_PREFIX = "ext."

ext_component_types: dict[str, Any] = {}


# adapted from rerun.log._add_extension_components
def _add_extension_components(
    instanced: dict[str, Component],
    splats: dict[str, Component],
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
                "Error converting extension data to arrow for component {}. Dropping.\n{}: {}".format(
                    name, type(ex).__name__, ex
                ),
                1,
            )
            continue

        is_splat = (len(np_value) == 1) and (len(identifiers or []) != 1)

        if is_splat:
            splats[name] = pa_value  # noqa
        else:
            instanced[name] = pa_value  # noqa


def log_any(
    entity_path: str,
    entity: Archetype,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    from .. import strict_mode

    if strict_mode():
        if not isinstance(entity, Archetype):
            raise TypeError(f"Expected Archetype, got {type(entity)}")

    # 0 = instanced, 1 = splat
    instanced: dict[str, Component] = {}
    splats: dict[str, Component] = {}

    for fld in fields(entity):
        if "component" in fld.metadata:
            comp: Component = getattr(entity, fld.name)
            if fld.metadata["component"] == "primary":
                instanced[comp.extension_name] = comp.storage
            elif len(comp) == 1:
                splats[comp.extension_name] = comp.storage
            elif len(comp) > 1:
                instanced[comp.extension_name] = comp.storage

    if ext:
        _add_extension_components(instanced, splats, ext, None)

    if splats:
        splats["rerun.instance_key"] = InstanceKeyArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless, recording=recording)

    # Always the primary component last so range-based queries will include the other data. See(#1215)
    bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless, recording=recording)
