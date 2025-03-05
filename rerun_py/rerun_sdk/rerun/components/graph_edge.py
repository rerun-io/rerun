# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/graph_edge.fbs".

# You can extend this class by creating a "GraphEdgeExt" class in "graph_edge_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)

__all__ = ["GraphEdge", "GraphEdgeBatch"]


class GraphEdge(datatypes.Utf8Pair, ComponentMixin):
    """**Component**: An edge in a graph connecting two nodes."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of GraphEdgeExt in graph_edge_ext.py

    # Note: there are no fields here because GraphEdge delegates to datatypes.Utf8Pair


class GraphEdgeBatch(datatypes.Utf8PairBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.GraphEdge")


# This is patched in late to avoid circular dependencies.
GraphEdge._BATCH_TYPE = GraphEdgeBatch  # type: ignore[assignment]
