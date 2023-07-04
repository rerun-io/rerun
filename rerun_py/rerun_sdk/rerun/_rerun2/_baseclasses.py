from __future__ import annotations

from typing import Any, Generic, Self, TypeVar, cast

import pyarrow as pa
from attrs import define, fields

T = TypeVar("T")


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


class BaseExtensionArray(NamedExtensionArray, Generic[T]):  # type: ignore[misc]
    """Extension array for datatypes and non-delegating components."""

    _EXTENSION_TYPE = pa.ExtensionType
    """The extension type class associated with this class."""

    @classmethod
    def from_similar(cls, data: T | None) -> Self:
        data_type = cls._EXTENSION_TYPE()

        if data is None:
            return cast(Self, data_type.wrap_array(pa.array([], type=data_type.storage_type)))
        else:
            return cast(Self, data_type.wrap_array(cls._native_to_pa_array(data, data_type.storage_type)))

    @staticmethod
    def _native_to_pa_array(data: T, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError


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


class BaseDelegatingExtensionArray(BaseExtensionArray[T]):  # type: ignore[misc]
    """Extension array for delegating components."""

    _DELEGATED_ARRAY_TYPE = BaseExtensionArray[T]  # type: ignore[valid-type]
    """The extension array class associated with this component's datatype."""

    @classmethod
    def from_similar(cls, data: T | None) -> Self:
        arr = cls._DELEGATED_ARRAY_TYPE.from_similar(data)

        # TODO(ab, cmc): we unwrap the type here because we can't have two layers of extension types for now
        return cast(Self, cls._EXTENSION_TYPE().wrap_array(arr.storage))
