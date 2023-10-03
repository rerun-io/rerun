from __future__ import annotations

from typing import Any, Iterable

import numpy as np
import pyarrow as pa

from ._log import AsComponents, ComponentBatchLike
from .error_utils import _send_warning

ANY_VALUE_TYPE_REGISTRY: dict[str, Any] = {}

COMPONENT_PREFIX = "any.value."


class AnyBatchValue(ComponentBatchLike):
    """
    Helper to log arbitrary data as a component batch.

    This is a very simple helper that implements the `ComponentBatchLike` interface on top
    of the `pyarrow` library array conversion functions.

    See also [rerun.AnyValues][].
    """

    def __init__(self, name: str, value: Any) -> None:
        """
        Construct a new AnyBatchValue.

        The component will be named "user.components.<NAME>".

        The value will be attempted to be converted into an arrow array by first calling
        the `as_arrow_array()` method if it's defined. All Rerun Batch datatypes implement
        this function so it's possible to pass them directly to AnyValues.

        If the object doesn't implement `as_arrow_array()`, it will be passed as an argument
        to [pyarrow.array](https://arrow.apache.org/docs/python/generated/pyarrow.array.html).

        Note: rerun requires that a given component only take on a single type.
        The first type logged will be the type that is used for all future logs
        of that component. The API will make a best effort to do type conversion
        if supported by numpy and arrow. Any components that can't be converted
        will be dropped, and a warning will be sent to the log.

        If you are want to inspect how your component will be converted to the
        underlying arrow code, the following snippet is what is happening
        internally:

        ```
        np_value = np.atleast_1d(np.array(value, copy=False))
        pa_value = pa.array(value)
        ```

        Parameters
        ----------
        name:
            The name of the component.
        value:
            The data to be logged as a component.
        """
        np_type, pa_type = ANY_VALUE_TYPE_REGISTRY.get(name, (None, None))

        self.name = name
        self.pa_array = None

        try:
            if hasattr(value, "as_arrow_array"):
                self.pa_array = value.as_arrow_array()
            else:
                if np_type is not None:
                    if value is None:
                        value = []
                    np_value = np.atleast_1d(np.array(value, copy=False, dtype=np_type))
                    self.pa_array = pa.array(np_value, type=pa_type)
                else:
                    if value is None:
                        _send_warning(f"AnyValues '{name}' of unknown type has no data. Ignoring.", 1)
                    else:
                        np_value = np.atleast_1d(np.array(value, copy=False))
                        self.pa_array = pa.array(np_value)
                        ANY_VALUE_TYPE_REGISTRY[name] = (np_value.dtype, self.pa_array.type)

        except Exception as ex:
            _send_warning(
                f"Error converting data to arrow for AnyValues '{name}'. Ignoring.\n{type(ex).__name__}: {ex}",
                1,
            )

    def is_valid(self) -> bool:
        return self.pa_array is not None

    def component_name(self) -> str:
        return COMPONENT_PREFIX + self.name

    def as_arrow_array(self) -> pa.Array | None:
        return self.pa_array


class AnyValues(AsComponents):
    """Helper to log arbitrary values as a bundle of components."""

    def __init__(self, **kwargs: Any) -> None:
        """
        Construct a new AnyValues bundle.

        Each kwarg will be logged as a separate component using the provided data.
         - The key will be used as the name of the component
         - The value must be able to be converted to an array of arrow types. In general, if
           you can pass it to [pyarrow.array](https://arrow.apache.org/docs/python/generated/pyarrow.array.html),
           you can log it as a extension component.

        All values must either have the same length, or be singular in which case they will be
        treated as a splat.

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

        Example
        -------
        ```
        rr.log(
            "any_values",
            rr.AnyValues(
                foo=[1.2, 3.4, 5.6],
                bar="hello world",
            ),
        )
        ```
        """
        global ANY_VALUE_TYPE_REGISTRY

        self.component_batches = []

        for name, value in kwargs.items():
            batch = AnyBatchValue(name, value)
            if batch.is_valid():
                self.component_batches.append(batch)

    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        return self.component_batches
