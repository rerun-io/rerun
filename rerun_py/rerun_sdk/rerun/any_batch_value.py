from __future__ import annotations

from collections.abc import Sized
from dataclasses import dataclass
from typing import Any

import numpy as np
import pyarrow as pa
from pyarrow import ArrowInvalid

from rerun._baseclasses import ComponentDescriptor

from ._baseclasses import ComponentBatchLike, ComponentColumn
from .error_utils import _send_warning_or_raise, catch_and_log_exceptions, strict_mode as strict_mode


@dataclass(frozen=True)
class ArrowTypeOnly:
    """Arrow type inferred directly from the input (dlpack, string, scalar)."""

    pa_type: pa.DataType


@dataclass(frozen=True)
class NumpyArrowType:
    """Arrow type inferred via numpy fallback, retaining the numpy dtype for future conversions."""

    np_type: np.dtype[Any]
    pa_type: pa.DataType


TypeRegistryValue = ArrowTypeOnly | NumpyArrowType


class TypeRegistry:
    """
    Registry that caches the inferred Arrow type for dynamic components.

    Rerun requires that a given component only takes on a single type. This registry
    records the type inferred on first log and reuses it for subsequent logs. A warning
    is emitted if a component's type changes.
    """

    def __init__(self) -> None:
        self._entries: dict[ComponentDescriptor, TypeRegistryValue] = {}

    def get(self, descriptor: ComponentDescriptor) -> TypeRegistryValue | None:
        return self._entries.get(descriptor)

    def register(self, descriptor: ComponentDescriptor, new: TypeRegistryValue, *, expect_column: bool = False) -> None:
        # In column mode, the outer list dimension represents rows, not part of the data type.
        if expect_column and pa.types.is_list(new.pa_type):
            new = ArrowTypeOnly(new.pa_type.value_type)

        existing = self._entries.get(descriptor)
        if existing is not None and existing != new:
            _send_warning_or_raise(
                f"Type for '{descriptor}' changed from {existing} to {new}. "
                "Rerun requires that a given component only takes on a single type.",
                depth_to_user_code=3,
            )
        self._entries[descriptor] = new


ANY_VALUE_TYPE_REGISTRY = TypeRegistry()


def _parse_arrow_array(
    value: Any,
    *,
    cached: TypeRegistryValue | None = None,
) -> tuple[pa.Array, TypeRegistryValue]:
    """
    Parse a value into an Arrow array, returning the array and its inferred type registry entry.

    Registration in the type registry is the caller's responsibility.
    """
    pa_type = cached.pa_type if cached is not None else None
    np_type = cached.np_type if isinstance(cached, NumpyArrowType) else None

    possible_array = _try_parse_dlpack(value, pa_type=pa_type)
    if possible_array is not None:
        return possible_array
    possible_array = _try_parse_string(value, pa_type=pa_type)
    if possible_array is not None:
        return possible_array
    possible_array = _try_parse_scalar(value, pa_type=pa_type)
    if possible_array is not None:
        return possible_array
    return _fallback_parse(
        value,
        pa_type=pa_type,
        np_type=np_type,
    )


def _try_parse_dlpack(value: Any, *, pa_type: Any | None = None) -> tuple[pa.Array, TypeRegistryValue] | None:
    # If the value has a __dlpack__ method, we can convert it to numpy without copy
    # then to arrow
    if hasattr(value, "__dlpack__"):
        try:
            pa_array: pa.Array = pa.array(np.from_dlpack(value), type=pa_type)
            return pa_array, ArrowTypeOnly(pa_array.type)
        except (ArrowInvalid, TypeError, BufferError):
            pass
    return None


def _try_parse_string(value: Any, *, pa_type: Any | None = None) -> tuple[pa.Array, TypeRegistryValue] | None:
    # Special case: strings are iterables so pyarrow will not
    # handle them properly
    if not isinstance(value, (str, bytes)):
        try:
            pa_array = pa.array(value, type=pa_type)
            if strict_mode():
                assert pa_array.type != pa.null(), (
                    f"pa.array of value {value} and type {pa_type} resulted in type {pa_array.type}"
                )

            return pa_array, ArrowTypeOnly(pa_array.type)
        except (ArrowInvalid, TypeError):
            pass
    return None


def _try_parse_scalar(value: Any, *, pa_type: Any | None = None) -> tuple[pa.Array, TypeRegistryValue] | None:
    try:
        pa_scalar = pa.scalar(value)
        pa_array = pa.array([pa_scalar], type=pa_type)
        return pa_array, ArrowTypeOnly(pa_array.type)
    except (ArrowInvalid, TypeError):
        pass
    return None


def _fallback_parse(
    value: Any,
    *,
    pa_type: Any | None = None,
    np_type: Any | None = None,
) -> tuple[pa.Array, TypeRegistryValue]:
    # Fall back - use numpy which handles a wide variety of lists, tuples,
    # and mixtures of them and will turn into a well formed array
    np_value = np.atleast_1d(np.asarray(value, dtype=np_type))
    try:
        pa_array = pa.array(np_value, type=pa_type)
    except pa.lib.ArrowInvalid as e:
        raise ValueError(f"Cannot convert {np_value} to arrow array of type {pa_type}.") from e
    except pa.lib.ArrowNotImplementedError as e:
        if np_type is None:
            raise ValueError(
                f"Cannot convert value {value} to arrow array of type {pa_type}."
                " Inconsistent with previous type provided."
            ) from e
        raise e
    return pa_array, NumpyArrowType(np_value.dtype, pa_array.type)


class AnyBatchValue(ComponentBatchLike):
    """
    Helper to log arbitrary data as a component batch or column.

    This is a very simple helper that implements the `ComponentBatchLike` interface on top
    of the `pyarrow` library array conversion functions.

    See also [rerun.AnyValues][].
    """

    def __init__(
        self,
        descriptor: str | ComponentDescriptor,
        value: Any,
        *,
        drop_untyped_nones: bool = True,
        expect_column: bool = False,
    ) -> None:
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
        expect_column:
            If True, the outermost dimension of the data is treated as the row/column dimension
            rather than as part of the data type. This prevents list-typed inference from being
            cached in the type registry (e.g., ``[[1,2,3], [4,5,6]]`` is treated as 2 rows of
            ``int64`` data rather than a single batch of ``list<int64>``).

        """
        if isinstance(descriptor, str):
            descriptor = ComponentDescriptor(descriptor)
        elif isinstance(descriptor, ComponentDescriptor):
            descriptor = descriptor

        self.descriptor = descriptor
        self.pa_array: pa.Array | None = None

        with catch_and_log_exceptions(f"Converting data for '{descriptor}'"):
            if isinstance(value, pa.Array):
                self.pa_array = value
            elif hasattr(value, "as_arrow_array"):
                self.pa_array = value.as_arrow_array()
            else:
                cached = ANY_VALUE_TYPE_REGISTRY.get(descriptor)
                if cached is None:
                    if value is None or (isinstance(value, Sized) and len(value) == 0):
                        if not drop_untyped_nones:
                            raise ValueError(f"Cannot convert {value} to arrow array without an explicit type")
                    else:
                        self.pa_array, inferred = _parse_arrow_array(value, cached=cached)
                        ANY_VALUE_TYPE_REGISTRY.register(descriptor, inferred, expect_column=expect_column)
                else:
                    if value is None:
                        value = []
                    self.pa_array, inferred = _parse_arrow_array(value, cached=cached)
                    ANY_VALUE_TYPE_REGISTRY.register(descriptor, inferred, expect_column=expect_column)

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
    ) -> ComponentColumn | None:
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

        Returns
        -------
        The component column, or ``None`` if the value could not be converted.

        """
        inst = cls(descriptor, value, drop_untyped_nones=drop_untyped_nones, expect_column=True)

        if not inst.is_valid():
            return None

        pa_array = inst.as_arrow_array()
        if pa_array is not None and pa.types.is_list(pa_array.type):
            # The outer list dimension is the row dimension. Flatten it and pass
            # the list offsets directly to ComponentColumn.
            column_offsets = pa_array.offsets.to_numpy().astype(np.int32)
            inst.pa_array = pa_array.values
            return ComponentColumn(inst.descriptor, inst, offsets=column_offsets)

        return ComponentColumn(inst.descriptor, inst)
