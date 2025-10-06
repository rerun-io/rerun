from __future__ import annotations

from typing import TYPE_CHECKING, Any

from rerun._baseclasses import ComponentDescriptor

from ._baseclasses import ComponentColumn, ComponentColumnList, DescribedComponentBatch
from ._log import AsComponents
from .any_batch_value import AnyBatchValue
from .error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from collections.abc import Mapping


class DynamicArchetype(AsComponents):
    """
    Helper to log data as a dynamically defined archetype.

    Example
    -------
    ```python
    rr.log(
        "some_type", rr.DynamicArchetype(
            archetype="my_archetype",
            components = {
                confidence=[1.2, 3.4, 5.6],
                description="Bla bla bla…",
                # URIs will become clickable links
                homepage="https://www.rerun.io",
                repository="https://github.com/rerun-io/rerun",
            },
        ),
    )
    ```

    """

    def __init__(
        self, archetype: str, drop_untyped_nones: bool = True, components: Mapping[str, Any] | None = None
    ) -> None:
        """
        Construct a new DynamicArchetype.

        Each of the provided components will be logged as a separate component batch with the same archetype using the provided data.
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
        archetype:
            All values in this class will be grouped under this archetype.
        drop_untyped_nones:
            If True, any components that are either None or empty will be dropped unless they
            have been previously logged with a type.
        components:
            The components to be logged.

        """
        global ANY_VALUE_TYPE_REGISTRY

        self._component_batches: list[DescribedComponentBatch] = []
        self._archetype: str | None = None
        self._name = self.__class__.__name__

        self._optional_archetype(archetype, drop_untyped_nones, components)

    def _optional_archetype(
        self, archetype: str | None, drop_untyped_nones: bool = True, components: Mapping[str, Any] | None = None
    ) -> None:
        """Support more flexible initialization."""
        self._archetype = archetype

        with catch_and_log_exceptions(self._name):
            if not isinstance(drop_untyped_nones, bool) and drop_untyped_nones is not None:
                raise ValueError(
                    f"{self._name} components must be set using the components argument, "
                    "you've provided a positional argument of type {type(drop_untyped_nones)} "
                    "to our boolean flag."
                )

            if components is not None:
                for name, value in components.items():
                    descriptor = ComponentDescriptor(
                        component=name,
                    )
                    if self._archetype is not None:
                        descriptor = descriptor.with_builtin_archetype(
                            archetype=self._archetype,
                        )
                    batch = AnyBatchValue(descriptor, value, drop_untyped_nones=drop_untyped_nones)
                    if batch.is_valid():
                        self._component_batches.append(DescribedComponentBatch(batch, batch.descriptor))

    @classmethod
    def _default_without_archetype(cls, drop_untyped_nones: bool = True, **kwargs: Any) -> DynamicArchetype:
        """Directly construct an DynamicArchetype without the Archetype."""
        # Create an empty archetype
        archetype = cls(archetype="placeholder", drop_untyped_nones=drop_untyped_nones, components={})
        # Clear the archetype name
        archetype._archetype = None
        # populate
        archetype._optional_archetype(None, drop_untyped_nones, components=kwargs)
        return archetype

    def _with_name(self, name: str) -> None:
        """Override the name in errors if contained elsewhere."""
        self._name = name

    def _with_descriptor_internal(
        self, descriptor: ComponentDescriptor, value: Any, *, drop_untyped_nones: bool = True
    ) -> DynamicArchetype:
        """Adds a `Batch` to this `DynamicArchetype` bundle."""
        batch = AnyBatchValue(descriptor, value, drop_untyped_nones=drop_untyped_nones)
        if batch.is_valid():
            self._component_batches.append(DescribedComponentBatch(batch, batch.descriptor))
        return self

    def with_component_from_data(self, field: str, value: Any, *, drop_untyped_nones: bool = True) -> DynamicArchetype:
        """Adds a `Batch` to this `DynamicArchetype` bundle."""
        descriptor = ComponentDescriptor(component=field)
        if self._archetype is not None:
            descriptor = descriptor.with_builtin_archetype(self._archetype)
        return self._with_descriptor_internal(descriptor, value, drop_untyped_nones=drop_untyped_nones)

    def with_component_override(
        self, field: str, component_type: str, value: Any, *, drop_untyped_nones: bool = True
    ) -> DynamicArchetype:
        """Adds a `Batch` to this `DynamicArchetype` bundle with name and component type."""
        descriptor = ComponentDescriptor(component=field, component_type=component_type)
        if self._archetype is not None:
            descriptor = descriptor.with_builtin_archetype(self._archetype)
        return self._with_descriptor_internal(descriptor, value, drop_untyped_nones=drop_untyped_nones)

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        with catch_and_log_exceptions(self._name):
            if len(self._component_batches) == 0:
                raise ValueError("No valid component batches to return.")
        return self._component_batches

    @classmethod
    def columns(
        cls, archetype: str, drop_untyped_nones: bool = True, components: Mapping[str, Any] | None = None
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented DynamicArchetype bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Each of the components will be logged as a separate component column using the provided data.
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
        archetype:
            All values in this class will be grouped under this archetype.
        drop_untyped_nones:
            If True, any components that are either None or empty will be dropped unless they
            have been previously logged with a type.
        components:
            The components to be logged.

        """
        inst = cls(archetype, drop_untyped_nones, components)
        return ComponentColumnList([
            ComponentColumn(batch.component_descriptor(), batch) for batch in inst._component_batches
        ])
