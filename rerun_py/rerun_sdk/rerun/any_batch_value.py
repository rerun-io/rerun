from __future__ import annotations

from collections.abc import Sized
from typing import Any

import numpy as np
import pyarrow as pa
from pyarrow import ArrowInvalid

from rerun._baseclasses import ComponentDescriptor

from ._baseclasses import ComponentBatchLike, ComponentColumn
from .error_utils import catch_and_log_exceptions, strict_mode as strict_mode

ANY_VALUE_TYPE_REGISTRY: dict[ComponentDescriptor, Any] = {}


def _parse_arrow_array(
    value: Any,
    *,
    pa_type: Any | None = None,
    np_type: Any | None = None,
    descriptor: ComponentDescriptor | None = None,
) -> pa.Array:
    possible_array = _try_parse_dlpack(value, pa_type=pa_type, descriptor=descriptor)
    if possible_array is not None:
        return possible_array
    possible_array = _try_parse_string(value, pa_type=pa_type, descriptor=descriptor)
    if possible_array is not None:
        return possible_array
    possible_array = _try_parse_scalar(value, pa_type=pa_type, descriptor=descriptor)
    if possible_array is not None:
        return possible_array
    return _fallback_parse(
        value,
        pa_type=pa_type,
        np_type=np_type,
        descriptor=descriptor,
    )


def _try_parse_dlpack(
    value: Any, *, pa_type: Any | None = None, descriptor: ComponentDescriptor | None = None
) -> pa.Array | None:
    # If the value has a __dlpack__ method, we can convert it to numpy without copy
    # then to arrow
    if hasattr(value, "__dlpack__"):
        try:
            pa_array: pa.Array = pa.array(np.from_dlpack(value), type=pa_type)
            if descriptor is not None:
                ANY_VALUE_TYPE_REGISTRY[descriptor] = (None, pa_array.type)
            return pa_array
        except (ArrowInvalid, TypeError, BufferError):
            pass
    return None


def _try_parse_string(
    value: Any, *, pa_type: Any | None = None, descriptor: ComponentDescriptor | None = None
) -> pa.Array | None:
    # Special case: strings are iterables so pyarrow will not
    # handle them properly
    if not isinstance(value, (str, bytes)):
        try:
            pa_array = pa.array(value, type=pa_type)
            if strict_mode():
                assert pa_array.type != pa.null(), (
                    f"pa.array of value {value} and type {pa_type} resulted in type {pa_array.type}"
                )

            if descriptor is not None:
                ANY_VALUE_TYPE_REGISTRY[descriptor] = (None, pa_array.type)
            return pa_array
        except (ArrowInvalid, TypeError):
            pass
    return None


def _try_parse_scalar(
    value: Any, *, pa_type: Any | None = None, descriptor: ComponentDescriptor | None = None
) -> pa.Array | None:
    try:
        pa_scalar = pa.scalar(value)
        pa_array = pa.array([pa_scalar], type=pa_type)
        if descriptor is not None:
            ANY_VALUE_TYPE_REGISTRY[descriptor] = (None, pa_array.type)
        return pa_array
    except (ArrowInvalid, TypeError):
        pass
    return None


def _fallback_parse(
    value: Any,
    *,
    pa_type: Any | None = None,
    np_type: Any | None = None,
    descriptor: ComponentDescriptor | None = None,
) -> pa.Array:
    # Fall back - use numpy which handles a wide variety of lists, tuples,
    # and mixtures of them and will turn into a well formed array
    np_value = np.atleast_1d(np.asarray(value, dtype=np_type))
    try:
        pa_array = pa.array(np_value, type=pa_type)
    except pa.lib.ArrowInvalid as e:
        # Improve the error message a bit:
        raise ValueError(f"Cannot convert {np_value} to arrow array of type {pa_type}. descriptor: {descriptor}") from e
    except pa.lib.ArrowNotImplementedError as e:
        if np_type is None and descriptor is None:
            raise ValueError(
                f"Cannot convert value {value} to arrow array of type {pa_type}."
                " Inconsistent with previous type provided."
            ) from e
        raise e
    if descriptor is not None:
        ANY_VALUE_TYPE_REGISTRY[descriptor] = (np_value.dtype, pa_array.type)
    return pa_array


class AnyBatchValue(ComponentBatchLike):
    """
    Helper to log arbitrary data as a component batch or column.

    This is a very simple helper that implements the `ComponentBatchLike` interface on top
    of the `pyarrow` library array conversion functions.

    See also [rerun.AnyValues][].
    """

    def __init__(self, descriptor: str | ComponentDescriptor, value: Any, *, drop_untyped_nones: bool = True) -> None:
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
        underlying arrow code, we first attempt to cast it directly to a pyarrow
        array. Failing this, we call

        ```
        pa_scalar = pa.scalar(value)
        pa_value = pa.array(pa_scalar)
        ```

        Parameters
        ----------
        descriptor:
            Either the name or the full descriptor of the component.
        value:
            The data to be logged as a component.
        drop_untyped_nones:
            If True, any components that are either None or empty will be dropped unless they have been
            previously logged with a type.

        """
        if isinstance(descriptor, str):
            descriptor = ComponentDescriptor(descriptor)
        elif isinstance(descriptor, ComponentDescriptor):
            descriptor = descriptor

        np_type, pa_type = ANY_VALUE_TYPE_REGISTRY.get(descriptor, (None, None))

        self.descriptor = descriptor
        self.pa_array = None

        with catch_and_log_exceptions(f"Converting data for '{descriptor}'"):
            if isinstance(value, pa.Array):
                self.pa_array = value
            elif hasattr(value, "as_arrow_array"):
                self.pa_array = value.as_arrow_array()
            else:
                if pa_type is None:
                    if value is None or (isinstance(value, Sized) and len(value) == 0):
                        if not drop_untyped_nones:
                            raise ValueError(f"Cannot convert {value} to arrow array without an explicit type")
                    else:
                        self.pa_array = _parse_arrow_array(value, pa_type=None, np_type=np_type, descriptor=descriptor)
                else:
                    if value is None:
                        value = []
                    self.pa_array = _parse_arrow_array(value, pa_type=pa_type, np_type=np_type, descriptor=None)

    def is_valid(self) -> bool:
        return self.pa_array is not None

    def component_descriptor(self) -> ComponentDescriptor:
        return self.descriptor

    def as_arrow_array(self) -> pa.Array | None:
        return self.pa_array

    @classmethod
    def column(
        cls,
        descriptor: str | ComponentDescriptor,
        value: Any,
        drop_untyped_nones: bool = True,
    ) -> ComponentColumn:
        """
        Construct a new column-oriented AnyBatchValue.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumn.partition` to repartition the data as needed.

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

        If you want to inspect how your component will be converted to the
        underlying arrow code, the following snippet is what is happening
        internally:

        ```
        np_value = np.atleast_1d(np.array(value, copy=False))
        pa_value = pa.array(value)
        ```

        Parameters
        ----------
        descriptor:
            Either the name or the full descriptor of the component.
        value:
            The data to be logged as a component.
        drop_untyped_nones:
            If True, any components that are either None or empty will be dropped unless they have been
            previously logged with a type.

        """
        inst = cls(descriptor, value, drop_untyped_nones=drop_untyped_nones)
        return ComponentColumn(descriptor, inst)
