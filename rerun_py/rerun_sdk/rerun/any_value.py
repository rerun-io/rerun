from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from pyarrow import ArrowInvalid

from rerun._baseclasses import ComponentDescriptor

from ._baseclasses import ComponentBatchLike, ComponentColumn, ComponentColumnList, DescribedComponentBatch
from ._log import AsComponents
from .error_utils import catch_and_log_exceptions

ANY_VALUE_TYPE_REGISTRY: dict[ComponentDescriptor, Any] = {}


class AnyBatchValue(ComponentBatchLike):
    """
    Helper to log arbitrary data as a component batch or column.

    This is a very simple helper that implements the `ComponentBatchLike` interface on top
    of the `pyarrow` library array conversion functions.

    See also [rerun.AnyValues][].
    """

    def __init__(self, descriptor: str | ComponentDescriptor, value: Any, drop_untyped_nones: bool = True) -> None:
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
            If True, any components that are None will be dropped unless they have been
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
                if pa_type is not None:
                    if value is None:
                        value = []
                    self._maybe_parse_dlpack(value, pa_type)
                    self._maybe_parse_string(value, pa_type)
                    self._maybe_parse_scalar(value, pa_type)
                    self._fallback_parse(value, pa_type, np_type)

                else:
                    if value is None:
                        if not drop_untyped_nones:
                            raise ValueError("Cannot convert None to arrow array. Type is unknown.")
                    else:
                        self._maybe_parse_dlpack(value, pa_type=None, descriptor=descriptor)
                        self._maybe_parse_string(value, pa_type=None, descriptor=descriptor)
                        self._maybe_parse_scalar(value, pa_type=None, descriptor=descriptor)
                        self._fallback_parse(value, pa_type, np_type, descriptor=descriptor)

    def _maybe_parse_dlpack(
        self, value: Any, pa_type: Any | None = None, descriptor: ComponentDescriptor | None = None
    ) -> None:
        # If the value has a __dlpack__ method, we can convert it to numpy without copy
        # then to arrow
        if self.pa_array is None and hasattr(value, "__dlpack__"):
            try:
                self.pa_array = pa.array(np.from_dlpack(value), type=pa_type)
                if descriptor is not None:
                    ANY_VALUE_TYPE_REGISTRY[descriptor] = (None, self.pa_array.type)
            except (ArrowInvalid, TypeError, BufferError):
                pass

    def _maybe_parse_string(
        self, value: Any, pa_type: Any | None = None, descriptor: ComponentDescriptor | None = None
    ) -> None:
        # Special case: strings are iterables so pyarrow will not
        # handle them properly
        if self.pa_array is None and not isinstance(value, (str, bytes)):
            try:
                self.pa_array = pa.array(value, type=pa_type)
                if descriptor is not None:
                    ANY_VALUE_TYPE_REGISTRY[descriptor] = (None, self.pa_array.type)
            except (ArrowInvalid, TypeError):
                pass

    def _maybe_parse_scalar(
        self, value: Any, pa_type: Any | None = None, descriptor: ComponentDescriptor | None = None
    ) -> None:
        if self.pa_array is None:
            try:
                pa_scalar = pa.scalar(value)
                self.pa_array = pa.array([pa_scalar], type=pa_type)
                if descriptor is not None:
                    ANY_VALUE_TYPE_REGISTRY[descriptor] = (None, self.pa_array.type)
            except (ArrowInvalid, TypeError):
                pass

    def _fallback_parse(
        self,
        value: Any,
        pa_type: Any | None = None,
        np_type: Any | None = None,
        descriptor: ComponentDescriptor | None = None,
    ) -> None:
        if self.pa_array is None:
            # Fall back - use numpy which handles a wide variety of lists, tuples,
            # and mixtures of them and will turn into a well formed array
            np_value = np.atleast_1d(np.asarray(value, dtype=np_type))
            try:
                self.pa_array = pa.array(np_value, type=pa_type)
            except pa.lib.ArrowNotImplementedError as e:
                if np_type is None and descriptor is None:
                    raise ValueError(
                        f"Cannot convert value {value} to arrow array of type {pa_type}."
                        " Inconsistent with previous type provided."
                    ) from e
            if descriptor is not None:
                ANY_VALUE_TYPE_REGISTRY[descriptor] = (np_value.dtype, self.pa_array.type)

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
            If True, any components that are None will be dropped unless they have been
            previously logged with a type.

        """
        inst = cls(descriptor, value, drop_untyped_nones)
        return ComponentColumn(descriptor, inst)


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
            # URIs will become clickable links
            homepage="https://www.rerun.io",
            repository="https://github.com/rerun-io/rerun",
        ),
    )
    ```

    """

    def __init__(self, drop_untyped_nones: bool = True, **kwargs: Any) -> None:
        """
        Construct a new AnyValues bundle.

        Each kwarg will be logged as a separate component batch using the provided data.
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
            if not isinstance(drop_untyped_nones, bool) and drop_untyped_nones is not None:
                raise ValueError(
                    "AnyValues components must be set using keyword arguments, "
                    "you've provided a positional argument of type {type(drop_untyped_nones)} "
                    "to our boolean flag."
                )

            for name, value in kwargs.items():
                batch = AnyBatchValue(name, value, drop_untyped_nones=drop_untyped_nones)
                if batch.is_valid():
                    self.component_batches.append(DescribedComponentBatch(batch, batch.descriptor))

    def with_field(
        self, descriptor: str | ComponentDescriptor, value: Any, drop_untyped_nones: bool = True
    ) -> AnyValues:
        """Adds an `AnyValueBatch` to this `AnyValues` bundle."""
        batch = AnyBatchValue(descriptor, value, drop_untyped_nones=drop_untyped_nones)
        if batch.is_valid():
            self.component_batches.append(DescribedComponentBatch(batch, batch.descriptor))
        return self

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        with catch_and_log_exceptions(self.__class__.__name__):
            if len(self.component_batches) == 0:
                raise ValueError("No valid component batches to return.")
        return self.component_batches

    @classmethod
    def columns(cls, drop_untyped_nones: bool = True, **kwargs: Any) -> ComponentColumnList:
        """
        Construct a new column-oriented AnyValues bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Each kwarg will be logged as a separate component column using the provided data.
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
        inst = cls(drop_untyped_nones, **kwargs)
        return ComponentColumnList([
            ComponentColumn(batch.component_descriptor(), batch) for batch in inst.component_batches
        ])
