# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/graph_edges.fbs".

# You can extend this class by creating a "GraphEdgesExt" class in "graph_edges_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["GraphEdges"]


@define(str=False, repr=False, init=False)
class GraphEdges(Archetype):
    """
    **Archetype**: A list of edges in a graph.

    By default, edges are undirected.

    ⚠️ **This is an experimental API! It is not fully supported, and is likely to change significantly in future versions.**
    """

    def __init__(self: Any, edges: datatypes.Utf8PairArrayLike, *, graph_type: components.GraphTypeLike | None = None):
        """
        Create a new instance of the GraphEdges archetype.

        Parameters
        ----------
        edges:
            A list of node IDs.
        graph_type:
            Specifies if the graph is directed or undirected.

            If no `GraphType` is provided, the graph is assumed to be undirected.

        """

        # You can define your own __init__ function as a member of GraphEdgesExt in graph_edges_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(edges=edges, graph_type=graph_type)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            edges=None,  # type: ignore[arg-type]
            graph_type=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> GraphEdges:
        """Produce an empty GraphEdges, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    edges: components.GraphEdgeBatch = field(
        metadata={"component": "required"},
        converter=components.GraphEdgeBatch._required,  # type: ignore[misc]
    )
    # A list of node IDs.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    graph_type: components.GraphTypeBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.GraphTypeBatch._optional,  # type: ignore[misc]
    )
    # Specifies if the graph is directed or undirected.
    #
    # If no `GraphType` is provided, the graph is assumed to be undirected.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
