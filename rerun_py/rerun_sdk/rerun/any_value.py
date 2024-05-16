from __future__ import annotations

from typing import Any, Iterable

import numpy as np
import pyarrow as pa

from ._log import AsComponents, ComponentBatchLike
from .error_utils import catch_and_log_exceptions

ANY_VALUE_TYPE_REGISTRY: dict[str, Any] = {}


class AnyBatchValue(ComponentBatchLike):
    """
    Helper to log arbitrary data as a component batch.

    This is a very simple helper that implements the `ComponentBatchLike` interface on top
    of the `pyarrow` library array conversion functions.

    See also [rerun.AnyValues][].
    """

    def __init__(self, name: str, value: Any, drop_untyped_nones: bool = True) -> None:
        """
        Construct a new AnyBatchValue.

        The value will be attempted to be converted into an arrow array by first calling
        the `as_arrow_array()` method if it's defined. All Rerun Batch datatypes implement
        this function so it's possible to pass them directly to AnyValues.

        If the object doesn't implement `as_arrow_array()`, it will be passed as an argument
        to [pyarrow.array][] .

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
        drop_untyped_nones:
            If True, any components that are None will be dropped unless they have been
            previously logged with a type.

        """
        np_type, pa_type = ANY_VALUE_TYPE_REGISTRY.get(name, (None, None))

        self.name = name
        self.pa_array = None

        with catch_and_log_exceptions(f"Converting data for '{name}'"):
            if isinstance(value, pa.Array):
                self.pa_array = value
            elif hasattr(value, "as_arrow_array"):
                self.pa_array = value.as_arrow_array()
            else:
                if np_type is not None:
                    if value is None:
                        value = []
                    np_value = np.atleast_1d(np.array(value, copy=False, dtype=np_type))
                    self.pa_array = pa.array(np_value, type=pa_type)
                else:
                    if value is None:
                        if not drop_untyped_nones:
                            raise ValueError("Cannot convert None to arrow array. Type is unknown.")
                    else:
                        np_value = np.atleast_1d(np.array(value, copy=False))
                        self.pa_array = pa.array(np_value)
                        ANY_VALUE_TYPE_REGISTRY[name] = (np_value.dtype, self.pa_array.type)

    def is_valid(self) -> bool:
        return self.pa_array is not None

    def component_name(self) -> str:
        return self.name

    def as_arrow_array(self) -> pa.Array | None:
        return self.pa_array


class AnyValues(AsComponents):
    """
    Helper to log arbitrary values as a bundle of components.

    Example
    -------
    ```python
    rr.log(
        "any_values", rr.AnyValues(
            confidence=[1.2, 3.4, 5.6],
            description="Bla bla bla…",
        ),
    )
    ```

    """

    def __init__(self, drop_untyped_nones: bool = True, **kwargs: Any) -> None:
        """
        Construct a new AnyValues bundle.

        Each kwarg will be logged as a separate component using the provided data.
         - The key will be used as the name of the component
         - The value must be able to be converted to an array of arrow types. In
           general, if you can pass it to [pyarrow.array][] you can log it as a
           extension component.

        Note: rerun requires that a given component only take on a single type.
        The first type logged will be the type that is used for all future logs
        of that component. The API will make a best effort to do type conversion
        if supported by numpy and arrow. Any components that can't be converted
        will result in a warning (or an exception in strict mode).

        `None` values provide a particular challenge as they have no type
        information until after the component has been logged with a particular
        type. By default, these values are dropped. This should generally be
        fine as logging `None` to clear the value before it has been logged is
        meaningless unless you are logging out-of-order data. In such cases,
        consider introducing your own typed component via
        [rerun.ComponentBatchLike][].

        You can change this behavior by setting `drop_untyped_nones` to `False`,
        but be aware that this will result in potential warnings (or exceptions
        in strict mode).

        If you are want to inspect how your component will be converted to the
        underlying arrow code, the following snippet is what is happening
        internally:
        ```
        np_value = np.atleast_1d(np.array(value, copy=False))
        pa_value = pa.array(value)
        ```

        Parameters
        ----------
        drop_untyped_nones:
            If True, any components that are None will be dropped unless they
            have been previously logged with a type.
        kwargs:
            The components to be logged.

        """
        global ANY_VALUE_TYPE_REGISTRY

        self.component_batches = []

        with catch_and_log_exceptions(self.__class__.__name__):
            for name, value in kwargs.items():
                batch = AnyBatchValue(name, value, drop_untyped_nones=drop_untyped_nones)
                if batch.is_valid():
                    self.component_batches.append(batch)

    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        return self.component_batches
