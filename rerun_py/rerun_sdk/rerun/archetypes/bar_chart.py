# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/bar_chart.fbs".

# You can extend this class by creating a "BarChartExt" class in "bar_chart_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions
from .bar_chart_ext import BarChartExt

__all__ = ["BarChart"]


@define(str=False, repr=False, init=False)
class BarChart(BarChartExt, Archetype):
    """
    **Archetype**: A bar chart.

    The x values will be the indices of the array, and the bar heights will be the provided values.

    Example
    -------
    ### Simple bar chart:
    ```python
    import rerun as rr

    rr.init("rerun_example_bar_chart", spawn=True)
    rr.log("bar_chart", rr.BarChart([8, 4, 0, 9, 1, 4, 1, 6, 9, 0]))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/1200w.png">
      <img src="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(self: Any, values: datatypes.TensorDataLike, *, color: datatypes.Rgba32Like | None = None) -> None:
        """
        Create a new instance of the BarChart archetype.

        Parameters
        ----------
        values:
            The values. Should always be a 1-dimensional tensor (i.e. a vector).
        color:
            The color of the bar chart

        """

        # You can define your own __init__ function as a member of BarChartExt in bar_chart_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(values=values, color=color)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            values=None,
            color=None,
        )

    @classmethod
    def _clear(cls) -> BarChart:
        """Produce an empty BarChart, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        values: datatypes.TensorDataLike | None = None,
        color: datatypes.Rgba32Like | None = None,
    ) -> BarChart:
        """
        Update only some specific fields of a `BarChart`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        values:
            The values. Should always be a 1-dimensional tensor (i.e. a vector).
        color:
            The color of the bar chart

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "values": values,
                "color": color,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> BarChart:
        """Clear all the fields of a `BarChart`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        values: datatypes.TensorDataArrayLike | None = None,
        color: datatypes.Rgba32ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        values:
            The values. Should always be a 1-dimensional tensor (i.e. a vector).
        color:
            The color of the bar chart

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                values=values,
                color=color,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {"values": values, "color": color}
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]
                elem_flat_len = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                if pa.types.is_fixed_size_list(arrow_array.type) and arrow_array.type.list_size == elem_flat_len:
                    # If the product of the last dimensions of the shape are equal to the size of the fixed size list array,
                    # we have `num_rows` single element batches (each element is a fixed sized list).
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

    values: components.TensorDataBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=BarChartExt.values__field_converter_override,  # type: ignore[misc]
    )
    # The values. Should always be a 1-dimensional tensor (i.e. a vector).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    color: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # The color of the bar chart
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
