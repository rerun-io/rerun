# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/bar_chart.fbs".

# You can extend this class by creating a "BarChartExt" class in "bar_chart_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["BarChart"]


@define(str=False, repr=False)
class BarChart(Archetype):
    """
    A Barchart.

    The x values will be the indices of the array, and the bar heights will be the provided values.
    """

    # You can define your own __init__ function as a member of BarChartExt in bar_chart_ext.py

    values: components.TensorDataArray = field(
        metadata={"component": "required"},
        converter=components.TensorDataArray.from_similar,  # type: ignore[misc]
    )
    """
    The values. Should always be a rank-1 tensor.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
