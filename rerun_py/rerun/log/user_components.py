from typing import Any, Dict, Optional, Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

# Fully qualified to avoid circular import
import rerun.log.error_utils
from rerun.components.instance import InstanceArray

from rerun import bindings

__all__ = [
    "_add_user_components",
    "log_user_components",
]

USER_PREFIX = "user."

USER_COMPONENT_TYPES: Dict[str, Any] = {}


def _add_user_components(
    instanced: Dict[str, Any],
    splats: Dict[str, Any],
    user_components: Dict[str, Any],
    identifiers: Optional[npt.NDArray[np.uint64]],
) -> None:
    for name, value in user_components.items():

        # Don't log empty components
        if value is None:
            continue

        # Add the user prefix, unless it's already there
        if not name.startswith(USER_PREFIX):
            name = USER_PREFIX + name

        np_type, pa_type = USER_COMPONENT_TYPES.get(name, (None, None))

        try:
            if np_type is not None:
                np_value = np.atleast_1d(np.array(value, copy=False, dtype=np_type))
                pa_value = pa.array(np_value, type=pa_type)
            else:
                np_value = np.atleast_1d(np.array(value, copy=False))
                pa_value = pa.array(np_value)
                USER_COMPONENT_TYPES[name] = (np_value.dtype, pa_value.type)
        except Exception as ex:
            rerun.log.error_utils._send_warning(
                "Error converting user data to arrow for component {}. Dropping.\n{}: {}".format(
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


def log_user_components(
    entity_path: str,
    user_components: Dict[str, Any],
    *,
    identifiers: Optional[Sequence[int]] = None,
    timeless: bool = False,
) -> None:
    """
    Log an arbitrary collection of user-defined components.

    Each item in `user_components` will be logged as a separate component.

     - The key will be used as the name of the component
     - The value must be able to be converted to an array of arrow types. In general, if
       you can pass it to [pyarrow.array](https://arrow.apache.org/docs/python/generated/pyarrow.array.html),
       you can log it as a user component.

    All values must either have the same length, or be singular in which case they will be
    treated as a splat.

    User components will be prefixed with "user." to avoid collisions with rerun native components.
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
        Path to the user component in the space hierarchy.
    user_components:
        A dictionary of user-defined components.
    identifiers:
        Optional identifiers for each component. If provided, must be the same length as the components.
    timeless:
        If true, the components will be timeless (default: False).

    """
    identifiers_np = np.array((), dtype="uint64")
    if identifiers:
        try:
            identifiers = [int(id) for id in identifiers]
            identifiers_np = np.array(identifiers, dtype="uint64")
        except ValueError:
            rerun.log.error_utils._send_warning("Only integer identifies supported", 1)

    instanced: Dict[str, Any] = {}
    splats: Dict[str, Any] = {}

    if len(identifiers_np):
        instanced["rerun.instance_key"] = InstanceArray.from_numpy(identifiers_np)

    _add_user_components(instanced, splats, user_components, identifiers_np)

    if instanced:
        bindings.log_arrow_msg(entity_path, components=instanced, timeless=timeless)

    if splats:
        splats["rerun.instance_key"] = InstanceArray.splat()
        bindings.log_arrow_msg(entity_path, components=splats, timeless=timeless)
