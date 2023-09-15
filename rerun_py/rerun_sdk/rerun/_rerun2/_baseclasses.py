from __future__ import annotations

from typing import TYPE_CHECKING, Any, Generic, Iterable, TypeVar, cast

import pyarrow as pa
from attrs import define, fields

T = TypeVar("T")

if TYPE_CHECKING:
    from .log import ComponentBatchLike


class IndicatorComponent:
    """Indicator component."""

    def __init__(self, archetype_name: str, num_instances: int) -> None:
        self._archetype_name = archetype_name
        self._num_instances = num_instances

    def component_name(self) -> str:
        return f"rerun.components.{self._archetype_name}Indicator"

    def as_arrow_batch(self) -> pa.Array:
        return pa.nulls(self._num_instances, type=pa.null())


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

    def archetype_name(self) -> str:
        return type(self).__name__

    def num_instances(self) -> int:
        for fld in fields(type(self)):
            if "component" in fld.metadata and fld.metadata["component"] == "primary":
                return len(getattr(self, fld.name))
        raise ValueError("Archetype has no primary component")

    def as_component_batches(self) -> Iterable[ComponentBatchLike]:
        """Extract the components from an entity, yielding (component, is_primary) tuples."""

        yield IndicatorComponent(self.archetype_name(), self.num_instances())

        for fld in fields(type(self)):
            if "component" in fld.metadata:
                comp = getattr(self, fld.name)
                # TODO(https://github.com/rerun-io/rerun/issues/3341): Depending on what we decide
                # to do with optional components, we may need to make this instead call `_empty_pa_array`
                if comp is not None:
                    yield comp

    __repr__ = __str__


class BaseExtensionType(pa.ExtensionType):  # type: ignore[misc]
    """Extension type for datatypes and non-delegating components."""

    _ARRAY_TYPE: type[pa.ExtensionArray] = pa.ExtensionArray
    """The extension array class associated with this class."""

    # Note: (de)serialization is not used in the Python SDK

    def __arrow_ext_serialize__(self) -> bytes:
        return b""

    # noinspection PyMethodOverriding
    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type: Any, serialized: Any) -> pa.ExtensionType:
        return cls()

    def __arrow_ext_class__(self) -> type[pa.ExtensionArray]:
        return self._ARRAY_TYPE


class NamedExtensionArray(pa.ExtensionArray):  # type: ignore[misc]
    """Common base class for any extension array that has a name."""

    _EXTENSION_NAME = ""
    """The fully qualified name of this class."""

    @property
    def extension_name(self) -> str:
        return self._EXTENSION_NAME


class BaseExtensionArray(NamedExtensionArray, Generic[T]):
    """Extension array for datatypes and non-delegating components."""

    _EXTENSION_TYPE = pa.ExtensionType
    """The extension type class associated with this class."""

    @classmethod
    def from_similar(cls, data: T | None) -> BaseExtensionArray[T] | None:
        """
        Primary method for creating Arrow arrays for components.

        This method must flexibly accept native data (which comply with type `T`). Subclasses must provide a type
        parameter specifying the type of the native data (this is automatically handled by the code generator).

        The actual creation of the Arrow array is delegated to the `_native_to_pa_array()` method, which is not
        implemented by default.

        Parameters
        ----------
        data : T | None
            The data to convert into an Arrow array.

        Returns
        -------
        The Arrow array encapsulating the data.
        """
        data_type = cls._EXTENSION_TYPE()

        if data is None:
            # TODO(https://github.com/rerun-io/rerun/issues/3341): Depending on what we decide
            # to do with optional components, we may want to rever this.
            # pa_array = _empty_pa_array(data_type.storage_type)
            return None
        else:
            pa_array = cls._native_to_pa_array(data, data_type.storage_type)
        return cast(BaseExtensionArray[T], data_type.wrap_array(pa_array))

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

    def component_name(self) -> str:
        return self.extension_name

    def as_arrow_batch(self) -> pa.Array:
        return self


def _empty_pa_array(type: pa.DataType) -> pa.Array:
    if isinstance(type, pa.ExtensionType):
        return _empty_pa_array(type.storage_type)

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


class BaseDelegatingExtensionType(pa.ExtensionType):  # type: ignore[misc]
    """Extension type for delegating components."""

    _TYPE_NAME = ""
    """The fully qualified name of the component."""

    _ARRAY_TYPE = pa.ExtensionArray
    """The extension array class associated with this component."""

    _DELEGATED_EXTENSION_TYPE = BaseExtensionType
    """The extension type class associated with this component's datatype."""

    def __init__(self) -> None:
        # TODO(ab, cmc): we unwrap the type here because we can't have two layers of extension types for now
        pa.ExtensionType.__init__(self, self._DELEGATED_EXTENSION_TYPE().storage_type, self._TYPE_NAME)

    # Note: (de)serialization is not used in the Python SDK

    def __arrow_ext_serialize__(self) -> bytes:
        return b""

    # noinspection PyMethodOverriding
    @classmethod
    def __arrow_ext_deserialize__(cls, storage_type: Any, serialized: Any) -> pa.ExtensionType:
        return cls()

    def __arrow_ext_class__(self) -> type[pa.ExtensionArray]:
        return self._ARRAY_TYPE  # type: ignore[no-any-return]


class BaseDelegatingExtensionArray(BaseExtensionArray[T]):
    """Extension array for delegating components."""

    _DELEGATED_ARRAY_TYPE = BaseExtensionArray[T]  # type: ignore[valid-type]
    """The extension array class associated with this component's datatype."""

    @classmethod
    def from_similar(cls, data: T | None) -> BaseDelegatingExtensionArray[T] | None:
        arr = cls._DELEGATED_ARRAY_TYPE.from_similar(data)
        # TODO(https://github.com/rerun-io/rerun/issues/3341): Depending on what we decide
        # to do with optional components, we may need to make this instead call `_empty_pa_array`
        if arr is None:
            return None

        # TODO(ab, cmc): we unwrap the type here because we can't have two layers of extension types for now
        return cast(BaseDelegatingExtensionArray[T], cls._EXTENSION_TYPE().wrap_array(arr.storage))
