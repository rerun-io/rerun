# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/graph_edges.fbs".

# You can extend this class by creating a "GraphEdgesExt" class in "graph_edges_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumn,
    DescribedComponentBatch,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["GraphEdges"]


@define(str=False, repr=False, init=False)
class GraphEdges(Archetype):
    """
    **Archetype**: A list of edges in a graph.

    By default, edges are undirected.

    Example
    -------
    ### Simple directed graph:
    ```python
    import rerun as rr

    rr.init("rerun_example_graph_directed", spawn=True)

    rr.log(
        "simple",
        rr.GraphNodes(
            node_ids=["a", "b", "c"], positions=[(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)], labels=["A", "B", "C"]
        ),
        rr.GraphEdges(edges=[("a", "b"), ("b", "c"), ("c", "a")], graph_type="directed"),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/1200w.png">
      <img src="https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(self: Any, edges: datatypes.Utf8PairArrayLike, *, graph_type: components.GraphTypeLike | None = None):
        """
        Create a new instance of the GraphEdges archetype.

        Parameters
        ----------
        edges:
            A list of node tuples.
        graph_type:
            Specifies if the graph is directed or undirected.

            If no [`components.GraphType`][rerun.components.GraphType] is provided, the graph is assumed to be undirected.

        """

        # You can define your own __init__ function as a member of GraphEdgesExt in graph_edges_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(edges=edges, graph_type=graph_type)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            edges=None,
            graph_type=None,
        )

    @classmethod
    def _clear(cls) -> GraphEdges:
        """Produce an empty GraphEdges, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        edges: datatypes.Utf8PairArrayLike | None = None,
        graph_type: components.GraphTypeLike | None = None,
    ) -> GraphEdges:
        """
        Update only some specific fields of a `GraphEdges`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        edges:
            A list of node tuples.
        graph_type:
            Specifies if the graph is directed or undirected.

            If no [`components.GraphType`][rerun.components.GraphType] is provided, the graph is assumed to be undirected.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "edges": edges,
                "graph_type": graph_type,
            }

            if clear:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def clear_fields(cls) -> GraphEdges:
        """Clear all the fields of a `GraphEdges`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            edges=[],
            graph_type=[],
        )
        return inst

    @classmethod
    def columns(
        cls,
        *,
        _lengths: npt.ArrayLike | None = None,
        edges: datatypes.Utf8PairArrayLike | None = None,
        graph_type: components.GraphTypeArrayLike | None = None,
    ) -> list[ComponentColumn]:
        """
        Partitions the component data into multiple sub-batches.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        If specified, `_lengths` must sum to the total length of the component batch.
        If left unspecified, it will default to unit-length batches.

        Parameters
        ----------
        edges:
            A list of node tuples.
        graph_type:
            Specifies if the graph is directed or undirected.

            If no [`components.GraphType`][rerun.components.GraphType] is provided, the graph is assumed to be undirected.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                edges=edges,
                graph_type=graph_type,
            )

        batches = [batch for batch in inst.as_component_batches() if isinstance(batch, DescribedComponentBatch)]
        if len(batches) == 0:
            return []

        if _lengths is None:
            _lengths = np.ones(len(batches[0]._batch.as_arrow_array()))

        columns = [batch.partition(_lengths) for batch in batches]

        indicator_batch = DescribedComponentBatch(cls.indicator(), cls.indicator().component_descriptor())
        indicator_column = indicator_batch.partition(np.zeros(len(_lengths)))  # type: ignore[arg-type]

        return [indicator_column] + columns

    edges: components.GraphEdgeBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.GraphEdgeBatch._converter,  # type: ignore[misc]
    )
    # A list of node tuples.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    graph_type: components.GraphTypeBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.GraphTypeBatch._converter,  # type: ignore[misc]
    )
    # Specifies if the graph is directed or undirected.
    #
    # If no [`components.GraphType`][rerun.components.GraphType] is provided, the graph is assumed to be undirected.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
