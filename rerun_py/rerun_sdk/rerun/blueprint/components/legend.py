# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/legend.fbs".

# You can extend this class by creating a "LegendExt" class in "legend_ext.py".

from __future__ import annotations

from ..._baseclasses import ComponentBatchMixin
from .. import datatypes

__all__ = ["Legend", "LegendBatch", "LegendType"]


class Legend(datatypes.Legend):
    """**Component**: Configuration for the legend of a plot."""

    # You can define your own __init__ function as a member of LegendExt in legend_ext.py

    # Note: there are no fields here because Legend delegates to datatypes.Legend
    pass


class LegendType(datatypes.LegendType):
    _TYPE_NAME: str = "rerun.blueprint.components.Legend"


class LegendBatch(datatypes.LegendBatch, ComponentBatchMixin):
    _ARROW_TYPE = LegendType()
