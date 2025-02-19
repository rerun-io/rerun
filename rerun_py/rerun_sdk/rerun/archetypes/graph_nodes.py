# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/graph_nodes.fbs".

# You can extend this class by creating a "GraphNodesExt" class in "graph_nodes_ext.py".

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

__all__ = ["GraphNodes"]


@define(str=False, repr=False, init=False)
class GraphNodes(Archetype):
    """
    **Archetype**: A list of nodes in a graph with optional labels, colors, etc.

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

    def __init__(
        self: Any,
        node_ids: datatypes.Utf8ArrayLike,
        *,
        positions: datatypes.Vec2DArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
    ):
        """
        Create a new instance of the GraphNodes archetype.

        Parameters
        ----------
        node_ids:
            A list of node IDs.
        positions:
            Optional center positions of the nodes.
        colors:
            Optional colors for the boxes.
        labels:
            Optional text labels for the node.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        radii:
            Optional radii for nodes.

        """

        # You can define your own __init__ function as a member of GraphNodesExt in graph_nodes_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                node_ids=node_ids,
                positions=positions,
                colors=colors,
                labels=labels,
                show_labels=show_labels,
                radii=radii,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            node_ids=None,
            positions=None,
            colors=None,
            labels=None,
            show_labels=None,
            radii=None,
        )

    @classmethod
    def _clear(cls) -> GraphNodes:
        """Produce an empty GraphNodes, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        node_ids: datatypes.Utf8ArrayLike | None = None,
        positions: datatypes.Vec2DArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
    ) -> GraphNodes:
        """
        Update only some specific fields of a `GraphNodes`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        node_ids:
            A list of node IDs.
        positions:
            Optional center positions of the nodes.
        colors:
            Optional colors for the boxes.
        labels:
            Optional text labels for the node.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        radii:
            Optional radii for nodes.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "node_ids": node_ids,
                "positions": positions,
                "colors": colors,
                "labels": labels,
                "show_labels": show_labels,
                "radii": radii,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> GraphNodes:
        """Clear all the fields of a `GraphNodes`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        node_ids: datatypes.Utf8ArrayLike | None = None,
        positions: datatypes.Vec2DArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        node_ids:
            A list of node IDs.
        positions:
            Optional center positions of the nodes.
        colors:
            Optional colors for the boxes.
        labels:
            Optional text labels for the node.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        radii:
            Optional radii for nodes.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                node_ids=node_ids,
                positions=positions,
                colors=colors,
                labels=labels,
                show_labels=show_labels,
                radii=radii,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "node_ids": node_ids,
            "positions": positions,
            "colors": colors,
            "labels": labels,
            "show_labels": show_labels,
            "radii": radii,
        }
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[arg-type]
                shape = np.shape(param)

                batch_length = shape[1] if len(shape) > 1 else 1
                num_rows = shape[0] if len(shape) >= 1 else 1
                lengths = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                lengths = np.ones(len(arrow_array))

            columns.append(batch.partition(lengths))

        indicator_column = cls.indicator().partition(np.zeros(len(lengths)))
        return ComponentColumnList([indicator_column] + columns)

    node_ids: components.GraphNodeBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.GraphNodeBatch._converter,  # type: ignore[misc]
    )
    # A list of node IDs.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    positions: components.Position2DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.Position2DBatch._converter,  # type: ignore[misc]
    )
    # Optional center positions of the nodes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional colors for the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TextBatch._converter,  # type: ignore[misc]
    )
    # Optional text labels for the node.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    show_labels: components.ShowLabelsBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ShowLabelsBatch._converter,  # type: ignore[misc]
    )
    # Optional choice of whether the text labels should be shown by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RadiusBatch._converter,  # type: ignore[misc]
    )
    # Optional radii for nodes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
