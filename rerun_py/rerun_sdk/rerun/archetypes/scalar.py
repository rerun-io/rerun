# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/scalar.fbs".

# You can extend this class by creating a "ScalarExt" class in "scalar_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from attrs import define, field
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["Scalar"]


@deprecated("""since 0.23.0: Use `Scalars` instead.""")
@define(str=False, repr=False, init=False)
class Scalar(Archetype):
    """
    **Archetype**: A double-precision scalar, e.g. for use for time-series plots.

    The current timeline value will be used for the time/X-axis, hence scalars
    should not be static.

    When used to produce a plot, this archetype is used to provide the data that
    is referenced by [`archetypes.SeriesLines`][rerun.archetypes.SeriesLines] or [`archetypes.SeriesPoints`][rerun.archetypes.SeriesPoints]. You can do
    this by logging both archetypes to the same path, or alternatively configuring
    the plot-specific archetypes through the blueprint.

    ⚠️ **Deprecated since 0.23.0**: Use `Scalars` instead.

    Examples
    --------
    ### Update a scalar over time:
    ```python
    from __future__ import annotations

    import math

    import rerun as rr

    rr.init("rerun_example_scalar_row_updates", spawn=True)

    for step in range(64):
        rr.set_time("step", sequence=step)
        rr.log("scalars", rr.Scalars(math.sin(step / 10.0)))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1200w.png">
      <img src="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/full.png" width="640">
    </picture>
    </center>

    ### Update a scalar over time, in a single operation:
    ```python
    from __future__ import annotations

    import numpy as np
    import rerun as rr

    rr.init("rerun_example_scalar_column_updates", spawn=True)

    times = np.arange(0, 64)
    scalars = np.sin(times / 10.0)

    rr.send_columns(
        "scalars",
        indexes=[rr.TimeColumn("step", sequence=times)],
        columns=rr.Scalars.columns(scalars=scalars),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1200w.png">
      <img src="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(self: Any, scalar: datatypes.Float64Like) -> None:
        """
        Create a new instance of the Scalar archetype.

        Parameters
        ----------
        scalar:
            The scalar value to log.

        """

        # You can define your own __init__ function as a member of ScalarExt in scalar_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(scalar=scalar)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            scalar=None,
        )

    @classmethod
    @deprecated("""since 0.23.0: Use `Scalars` instead.""")
    def _clear(cls) -> Scalar:
        """Produce an empty Scalar, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    @deprecated("""since 0.23.0: Use `Scalars` instead.""")
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        scalar: datatypes.Float64Like | None = None,
    ) -> Scalar:
        """
        Update only some specific fields of a `Scalar`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        scalar:
            The scalar value to log.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "scalar": scalar,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> Scalar:
        """Clear all the fields of a `Scalar`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    @deprecated("""since 0.23.0: Use `Scalars` instead.""")
    def columns(
        cls,
        *,
        scalar: datatypes.Float64ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        scalar:
            The scalar value to log.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                scalar=scalar,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {"scalar": scalar}
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]

                if pa.types.is_fixed_size_list(arrow_array.type) and len(shape) <= 2:
                    # If shape length is 2 or less, we have `num_rows` single element batches (each element is a fixed sized list).
                    # `shape[1]` should be the length of the fixed sized list.
                    # (This should have been already validated by conversion to the arrow_array)
                    batch_length = 1
                else:
                    batch_length = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    scalar: components.ScalarBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ScalarBatch._converter,  # type: ignore[misc]
    )
    # The scalar value to log.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
