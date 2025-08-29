from __future__ import annotations

import warnings
from typing import Any

from rerun._baseclasses import ComponentDescriptor

from ._baseclasses import ComponentColumn, ComponentColumnList, DescribedComponentBatch
from ._log import AsComponents
from .archetype_builder import AnyBatchValue
from .error_utils import catch_and_log_exceptions

ANY_VALUE_TYPE_REGISTRY: dict[ComponentDescriptor, Any] = {}


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
        # TODO(#10908): Prune this type in 0.26
        if isinstance(descriptor, ComponentDescriptor):
            warnings.warn(
                "`rr.AnyValues.with_field` using a component descriptor is deprecated, "
                "use ArchetypeBuilder if trying to specify archetype grouping of values.",
                DeprecationWarning,
                stacklevel=2,
            )
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
