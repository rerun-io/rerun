from __future__ import annotations

from typing import Any, Generic, Iterable, Protocol, TypeVar

import pyarrow as pa
from attrs import define, fields

from .error_utils import catch_and_log_exceptions

T = TypeVar("T")


class ComponentBatchLike(Protocol):
    """Describes interface for objects that can be converted to batch of rerun Components."""

    def component_name(self) -> str:
        """Returns the name of the component."""
        ...

    def as_arrow_array(self) -> pa.Array:
        """
        Returns a `pyarrow.Array` of the component data.

        Each element in the array corresponds to an instance of the component. Single-instanced
        components and splats must still be represented as a 1-element array.
        """
        ...


class AsComponents(Protocol):
    """
    Describes interface for interpreting an object as a bundle of Components.

    Note: the `num_instances()` function is an optional part of this interface. The method does not need to be
    implemented as it is only used after checking for its existence. (There is unfortunately no way to express this
    correctly with the Python typing system, see https://github.com/python/typing/issues/601).
    """

    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        """
        Returns an iterable of `ComponentBatchLike` objects.

        Each object in the iterable must adhere to the `ComponentBatchLike`
        interface. All of the batches should have the same length as the value
        returned by `num_instances`, or length 1 if the component is a splat.,
        or 0 if the component is being cleared.
        """
        ...

    # def num_instances(self) -> int | None:
    #     """
    #     (Optional) The number of instances in each batch.
    #
    #     If not implemented, the number of instances will be determined by the longest
    #     batch in the bundle.
    #
    #     Each batch returned by `as_component_batches` should have this number of
    #     elements, or 1 in the case it is a splat, or 0 in the case that
    #     component is being cleared.
    #     """
    #     return None


@define
class Archetype:
    """Base class for all archetypes."""

    def __str__(self) -> str:
        cls = type(self)

        s = f"rr.{cls.__name__}(\n"
        for fld in fields(cls):
            if "component" in fld.metadata:
                comp = getattr(self, fld.name)
                datatype = getattr(comp, "type", None)
                if datatype:
                    s += f"  {datatype.extension_name}<{datatype.storage_type}>(\n    {comp.to_pylist()}\n  )\n"
        s += ")"

        return s

    @classmethod
    def archetype_name(cls) -> str:
        return "rerun.archetypes." + cls.__name__

    @classmethod
    def indicator(cls) -> ComponentBatchLike:
        """
        Creates a `ComponentBatchLike` out of the associated indicator component.

        This allows for associating arbitrary indicator components with arbitrary data.
        Check out the `manual_indicator` API example to see what's possible.
        """
        from ._log import IndicatorComponentBatch

        return IndicatorComponentBatch(cls.archetype_name())

    def num_instances(self) -> int:
        """
        The number of instances that make up the batch.

        Part of the `AsComponents` logging interface.
        """
        for fld in fields(type(self)):
            # TODO(jleibs): What to do if multiple required components have different lengths?
            if "component" in fld.metadata and fld.metadata["component"] == "required":
                return len(getattr(self, fld.name))
        raise ValueError("Archetype has no required components")

    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        """
        Return all the component batches that make up the archetype.

        Part of the `AsComponents` logging interface.
        """
        yield self.indicator()

        for fld in fields(type(self)):
            if "component" in fld.metadata:
                comp = getattr(self, fld.name)
                # TODO(#3381): Depending on what we decide
                # to do with optional components, we may need to make this instead call `_empty_pa_array`
                if comp is not None:
                    yield comp

    __repr__ = __str__


class BaseExtensionType(pa.ExtensionType):  # type: ignore[misc]
    """Extension type for datatypes and non-delegating components."""

    _TYPE_NAME: str
    """The name used when constructing the extension type.

    Should following rerun typing conventions:
     - `rerun.datatypes.<TYPE>` for datatypes
     - `rerun.components.<TYPE>` for components

    Many component types simply subclass a datatype type and override
    the `_TYPE_NAME` field.
    """

    _ARRAY_TYPE: type[pa.ExtensionArray] = pa.ExtensionArray
    """The extension array class associated with this class."""

    # Note: (de)serialization is not used in the Python SDK

    def __arrow_ext_serialize__(self) -> bytes:
        return b""

    # noinspection PyMethodOverriding
    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type: Any, serialized: Any) -> pa.ExtensionType:
        return cls()


class BaseBatch(Generic[T]):
    _ARROW_TYPE: BaseExtensionType = None  # type: ignore[assignment]
    """The pyarrow type of this batch."""

    def __init__(self, data: T | None, strict: bool | None = None) -> None:
        """
        Construct a new batch.

        This method must flexibly accept native data (which comply with type `T`). Subclasses must provide a type
        parameter specifying the type of the native data (this is automatically handled by the code generator).

        A value of None indicates that the component should be cleared and results in the creation of an empty
        array.

        The actual creation of the Arrow array is delegated to the `_native_to_pa_array()` method, which is not
        implemented by default.

        Parameters
        ----------
        data : T | None
            The data to convert into an Arrow array.
        strict : bool | None
            Whether to raise an exception if the data cannot be converted into an Arrow array. If None, the value
            defaults to the value of the `rerun.strict` global setting.

        Returns
        -------
        The Arrow array encapsulating the data.
        """
        if data is not None:
            with catch_and_log_exceptions(self.__class__.__name__, strict=strict):
                # If data is already an arrow array, use it
                if isinstance(data, pa.Array) and data.type == self._ARROW_TYPE:
                    self.pa_array = data
                elif isinstance(data, pa.Array) and data.type == self._ARROW_TYPE.storage_type:
                    self.pa_array = self._ARROW_TYPE.wrap_array(data)
                else:
                    self.pa_array = self._ARROW_TYPE.wrap_array(
                        self._native_to_pa_array(data, self._ARROW_TYPE.storage_type)
                    )
                return

        # If we didn't return above, default to the empty array
        self.pa_array = _empty_pa_array(self._ARROW_TYPE)

    @classmethod
    def _required(cls, data: T | None) -> BaseBatch[T]:
        """
        Primary method for creating Arrow arrays for optional components.

        Just calls through to __init__, but with clearer type annotations.
        """
        return cls(data)

    @classmethod
    def _optional(cls, data: T | None) -> BaseBatch[T] | None:
        """
        Primary method for creating Arrow arrays for optional components.

        For optional components, the default value of None is preserved in the field to indicate that the optional
        field was not specified.
        If any value other than None is provided, it is passed through to `__init__`.

        Parameters
        ----------
        data : T | None
            The data to convert into an Arrow array.

        Returns
        -------
        The Arrow array encapsulating the data.
        """
        if data is None:
            return None
        else:
            return cls(data)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, BaseBatch):
            return NotImplemented
        return self.pa_array == other.pa_array  # type: ignore[no-any-return]

    def __len__(self) -> int:
        return len(self.pa_array)

    @staticmethod
    def _native_to_pa_array(data: T, data_type: pa.DataType) -> pa.Array:
        """
        Converts native data into an Arrow array.

        Subclasses must provide an implementation of this method (via an override) if they are to be used as either
        an archetype's field (which should be the case for all components), or a (delegating) component's field (for
        datatypes). Datatypes which are used only within other datatypes may omit implementing this method, provided
        that the top-level datatype implements it.

        A hand-coded override must be provided for the code generator to implement this method. The override must be
        named `native_to_pa_array_override()` and exist as a static member of the `<TYPE>Ext` class located in
        `<type>_ext.py`.

        `ColorExt.native_to_pa_array_override()` in `color_ext.py` is a good example of how to implement this method, in
        conjunction with the native type's converter (see `rgba__field_converter_override()`, used to construct the
        native `Color` object).

        Parameters
        ----------
        data : T
            The data to convert into an Arrow array.
        data_type : pa.DataType
            The Arrow data type of the data.

        Returns
        -------
        The Arrow array encapsulating the data.
        """
        raise NotImplementedError

    def as_arrow_array(self) -> pa.Array:
        """
        The component as an arrow batch.

        Part of the `ComponentBatchLike` logging interface.
        """
        return self.pa_array


class ComponentBatchMixin(ComponentBatchLike):
    def component_name(self) -> str:
        """
        The name of the component.

        Part of the `ComponentBatchLike` logging interface.
        """
        return self._ARROW_TYPE._TYPE_NAME  # type: ignore[attr-defined, no-any-return]


@catch_and_log_exceptions(context="creating empty array")
def _empty_pa_array(type: pa.DataType) -> pa.Array:
    if isinstance(type, pa.ExtensionType):
        return type.wrap_array(_empty_pa_array(type.storage_type))

    # Creation of empty arrays of dense unions aren't implemented in pyarrow yet.
    if isinstance(type, pa.UnionType):
        return pa.UnionArray.from_buffers(
            type=type,
            length=0,
            buffers=[
                None,
                pa.array([], type=pa.int8()).buffers()[1],
                pa.array([], type=pa.int32()).buffers()[1],
            ],
            children=[_empty_pa_array(field_type.type) for field_type in type],
        )
    # This also affects structs *containing* dense unions.
    if isinstance(type, pa.StructType):
        return pa.StructArray.from_arrays([_empty_pa_array(field_type.type) for field_type in type], fields=list(type))

    return pa.array([], type=type)
