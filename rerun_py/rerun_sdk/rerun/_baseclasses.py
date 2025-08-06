from __future__ import annotations

import re
from collections.abc import Iterable, Iterator
from typing import Generic, Protocol, TypeVar, runtime_checkable

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, fields
from rerun_bindings import ComponentDescriptor

from .error_utils import catch_and_log_exceptions

T = TypeVar("T")


class DescribedComponentBatch:
    """
    A `ComponentBatchLike` object with its associated `ComponentDescriptor`.

    Used by implementers of `AsComponents` to both efficiently expose their component data
    and assign the right tags given the surrounding context.
    """

    def __init__(self, batch: ComponentBatchLike, descriptor: ComponentDescriptor) -> None:
        self._batch = batch
        self._descriptor = descriptor

    def component_descriptor(self) -> ComponentDescriptor:
        """Returns the complete descriptor of the component."""
        return self._descriptor

    def as_arrow_array(self) -> pa.Array:
        """
        Returns a `pyarrow.Array` of the component data.

        Part of the `ComponentBatchLike` logging interface.
        """
        return self._batch.as_arrow_array()

    def partition(self, lengths: npt.ArrayLike | None = None) -> ComponentColumn:
        """
        Partitions the component batch into multiple sub-batches, forming a column.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumn.partition` to repartition the data as needed.

        Parameters
        ----------
        lengths:
            The offsets to partition the component at.
            If specified, `lengths` must sum to the total length of the component batch.
            If left unspecified, it will default to unit-length batches.

        Returns
        -------
        The partitioned component batch as a column.

        """
        return ComponentColumn(self._descriptor, self, lengths=lengths)


@runtime_checkable
class ComponentBatchLike(Protocol):
    """Describes interface for objects that can be converted to batch of rerun Components."""

    def as_arrow_array(self) -> pa.Array:
        """Returns a `pyarrow.Array` of the component data."""
        ...


@runtime_checkable
class AsComponents(Protocol):
    """Describes interface for interpreting an object as a bundle of Components."""

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        """
        Returns an iterable of `ComponentBatchLike` objects.

        Each object in the iterable must adhere to the `ComponentBatchLike` interface.
        """
        ...


@define
class Archetype(AsComponents):
    """Base class for all archetypes."""

    def __str__(self) -> str:
        from pprint import pformat

        cls = type(self)

        def fields_repr() -> Iterable[str]:
            for fld in fields(cls):
                if "component" in fld.metadata:
                    comp = getattr(self, fld.name)
                    if comp is None:
                        continue

                    as_arrow_array = getattr(comp, "as_arrow_array", None)

                    if as_arrow_array is None:
                        comp_contents = "<unknown>"
                    else:
                        # Note: the regex here is necessary because for some reason pformat add spurious spaces when
                        # indent > 1.
                        comp_contents = re.sub(
                            r"\[\s+\[", "[[", pformat(as_arrow_array().to_pylist(), compact=True, indent=4)
                        )

                    yield f"  {fld.name}={comp_contents}"

        args = ",\n".join(fields_repr())
        if args:
            return f"rr.{cls.__name__}(\n{args}\n)"
        else:
            return f"rr.{cls.__name__}()"

    @classmethod
    def archetype(cls) -> str:
        return ".".join(cls.__module__.rsplit(".", 1)[:-1] + [cls.__name__])

    @classmethod
    def archetype_short_name(cls) -> str:
        return cls.archetype().rsplit(".", 1)[-1]

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        """
        Return all the component batches that make up the archetype.

        Part of the `AsComponents` logging interface.
        """
        batches = []

        for fld in fields(type(self)):
            if "component" in fld.metadata:
                comp = getattr(self, fld.name)
                if comp is not None:
                    descr = ComponentDescriptor(
                        self.archetype_short_name() + ":" + fld.name,
                        component_type=comp._COMPONENT_TYPE,
                        archetype=self.archetype(),
                    )
                    batches.append(DescribedComponentBatch(comp, descr))

        return batches

    __repr__ = __str__


class BaseBatch(Generic[T]):
    _ARROW_DATATYPE: pa.DataType | None = None
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
                if isinstance(data, pa.Array) and data.type == self._ARROW_DATATYPE:
                    self.pa_array = data
                else:
                    self.pa_array = self._native_to_pa_array(data, self._ARROW_DATATYPE)
                return

        # If we didn't return above, default to the empty array
        self.pa_array = _empty_pa_array(self._ARROW_DATATYPE)

    @classmethod
    def _converter(cls, data: T | None) -> BaseBatch[T] | None:
        """
        Primary method for creating Arrow arrays for components.

        The default value of None is preserved in the field to indicate that the optional field was not specified.
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


class ComponentColumn:
    """
    A column of components that can be sent using `send_columns`.

    This is represented by a ComponentBatch array that has been partitioned into multiple segments.
    This is useful for reinterpreting a single contiguous batch as multiple sub-batches
    to use with the [`send_columns`][rerun.send_columns] API.
    """

    def __init__(
        self,
        descriptor: str | ComponentDescriptor,
        component_batch: ComponentBatchLike,
        *,
        lengths: npt.ArrayLike | None = None,
    ) -> None:
        """
        Construct a new component column.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned column will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumn.partition` to repartition the data as needed.

        Parameters
        ----------
        descriptor:
            The component descriptor for this component batch.
        component_batch:
            The component batch to partition into a column.

        lengths:
            The offsets to partition the component at.
            If specified, `lengths` must sum to the total length of the component batch.
            If left unspecified, it will default to unit-length batches.

        """
        if isinstance(descriptor, str):
            descriptor = ComponentDescriptor(descriptor)
        elif isinstance(descriptor, ComponentDescriptor):
            descriptor = descriptor

        self.descriptor = descriptor
        self.component_batch = component_batch

        if lengths is None:
            # Normal component, no lengths -> unit-sized batches by default
            self.lengths = np.ones(len(component_batch.as_arrow_array()), dtype=np.int32)
        else:
            # Normal component, lengths specified -> follow instructions
            lengths = np.array(lengths)
            if lengths.ndim != 1:
                raise ValueError("Lengths must be a 1D array.")
            self.lengths = lengths.flatten().astype(np.int32)

    def component_descriptor(self) -> ComponentDescriptor:
        """
        Returns the complete descriptor of the component.

        Part of the `ComponentBatchLike` logging interface.
        """
        return self.descriptor

    def as_arrow_array(self) -> pa.Array:
        """
        The component as an arrow batch.

        Part of the `ComponentBatchLike` logging interface.
        """
        array = self.component_batch.as_arrow_array()
        offsets = np.concatenate((np.array([0], dtype="int32"), np.cumsum(self.lengths, dtype="int32")))
        return pa.ListArray.from_arrays(offsets, array)

    def partition(self, lengths: npt.ArrayLike) -> ComponentColumn:
        """
        (Re)Partitions the column.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumn.partition` to repartition the data as needed.

        Parameters
        ----------
        lengths:
            The offsets to partition the component at.

        Returns
        -------
        The (re)partitioned column.

        """
        return ComponentColumn(self.descriptor, self.component_batch, lengths=lengths)


class ComponentColumnList(Iterable[ComponentColumn]):
    """
    A collection of [ComponentColumn][]s.

    Useful to partition and log multiple columns at once.
    """

    def __init__(self, columns: Iterable[ComponentColumn]) -> None:
        self._columns = list(columns)

    def __iter__(self) -> Iterator[ComponentColumn]:
        return iter(self._columns)

    def __len__(self) -> int:
        return len(self._columns)

    def partition(self, lengths: npt.ArrayLike) -> ComponentColumnList:
        """
        (Re)Partitions the columns.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumn.partition` to repartition the data as needed.

        Parameters
        ----------
        lengths:
            The offsets to partition the component at.
            If specified, `lengths` must sum to the total length of the component batch.
            If left unspecified, it will default to unit-length batches.

        Returns
        -------
        The partitioned component batch as a column.

        """
        return ComponentColumnList([col.partition(lengths) for col in self._columns])


class ComponentBatchMixin(ComponentBatchLike):
    def component_type(self) -> str:
        """
        Returns the name of the component.

        Part of the `ComponentBatchLike` logging interface.
        """
        return self._COMPONENT_TYPE  # type: ignore[attr-defined, no-any-return]

    def described(self, descriptor: ComponentDescriptor) -> DescribedComponentBatch:
        """Wraps the current `ComponentBatchLike` in a `DescribedComponentBatch` with the given descriptor."""
        return DescribedComponentBatch(self, descriptor)

    def partition(self, lengths: npt.ArrayLike | None = None) -> ComponentColumn:
        """
        Partitions the component batch into multiple sub-batches, forming a column.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumn.partition` to repartition the data as needed.

        Parameters
        ----------
        lengths:
            The offsets to partition the component at.
            If specified, `lengths` must sum to the total length of the component batch.
            If left unspecified, it will default to unit-length batches.

        Returns
        -------
        The partitioned component batch as a column.

        """
        return ComponentColumn(self.component_type(), self, lengths=lengths)


class ComponentMixin(ComponentBatchLike):
    """
    Makes components adhere to the `ComponentBatchLike` interface.

    A single component will always map to a batch of size 1.

    The class using the mixin must define the `_BATCH_TYPE` field, which should be a subclass of `BaseBatch`.
    """

    @classmethod
    def arrow_type(cls) -> pa.DataType:
        """
        The pyarrow type of this batch.

        Part of the `ComponentBatchLike` logging interface.
        """
        return cls._BATCH_TYPE._ARROW_DATATYPE  # type: ignore[attr-defined]

    def component_type(self) -> str:
        """
        Returns the name of the component.

        Part of the `ComponentBatchLike` logging interface.
        """
        return self._BATCH_TYPE._COMPONENT_TYPE  # type: ignore[attr-defined, no-any-return]

    def as_arrow_array(self) -> pa.Array:
        """
        The component as an arrow batch.

        Part of the `ComponentBatchLike` logging interface.
        """
        return self._BATCH_TYPE([self]).as_arrow_array()  # type: ignore[attr-defined]


@catch_and_log_exceptions(context="creating empty array")
def _empty_pa_array(type: pa.DataType) -> pa.Array:
    if type == pa.null():
        return pa.nulls(0)

    if isinstance(type, pa.ExtensionType):
        return type.wrap_array(_empty_pa_array(type.storage_type))

    # Creation of empty arrays of dense unions aren't implemented in pyarrow yet.
    if isinstance(type, pa.UnionType):
        if type.mode == "dense":
            return pa.UnionArray.from_buffers(
                type=type,
                length=0,
                buffers=[
                    None,
                    pa.array([], type=pa.int8()).buffers()[1],  # types
                    pa.array([], type=pa.int32()).buffers()[1],  # offsets
                ],
                children=[_empty_pa_array(field_type.type) for field_type in type],
            )
        else:
            return pa.UnionArray.from_buffers(
                type=type,
                length=0,
                buffers=[
                    None,
                    pa.array([], type=pa.int8()).buffers()[1],  # types
                ],
                children=[_empty_pa_array(field_type.type) for field_type in type],
            )

    # This also affects structs *containing* dense unions.
    if isinstance(type, pa.StructType):
        return pa.StructArray.from_arrays([_empty_pa_array(field_type.type) for field_type in type], fields=list(type))

    return pa.array([], type=type)
