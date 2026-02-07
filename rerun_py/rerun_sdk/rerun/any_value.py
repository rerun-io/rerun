from __future__ import annotations

from typing import Any

from ._baseclasses import ComponentColumn, ComponentColumnList, DescribedComponentBatch
from ._log import AsComponents
from .dynamic_archetype import DynamicArchetype


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
            If True, any components that are either None or empty will be dropped unless they
            have been previously logged with a type.
        kwargs:
            The components to be logged.

        """
        self._builder = DynamicArchetype._default_without_archetype(drop_untyped_nones, **kwargs)
        self._builder._with_name(self.__class__.__name__)

    def with_component_from_data(self, descriptor: str, value: Any, *, drop_untyped_nones: bool = True) -> AnyValues:
        """Adds an `AnyValueBatch` to this `AnyValues` bundle."""
        self._builder.with_component_from_data(descriptor, value, drop_untyped_nones=drop_untyped_nones)
        return self

    def with_component_override(
        self, field: str, component_type: str, value: Any, *, drop_untyped_nones: bool = True
    ) -> AnyValues:
        """Adds an `AnyValueBatch` to this `AnyValues` bundle with name and component type."""
        self._builder.with_component_override(field, component_type, value, drop_untyped_nones=drop_untyped_nones)
        return self

    def as_component_batches(self) -> list[DescribedComponentBatch]:
        return self._builder.as_component_batches()

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
            If True, any components that are either None or empty will be dropped unless they
            have been previously logged with a type.
        kwargs:
            The components to be logged.

        """
        inst = cls(drop_untyped_nones, **kwargs)
        return ComponentColumnList([
            ComponentColumn(batch.component_descriptor(), batch) for batch in inst._builder.as_component_batches()
        ])

    @property
    def component_batches(self) -> list[DescribedComponentBatch]:
        return self._builder.as_component_batches()
