# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/graph_nodes.fbs".

# You can extend this class by creating a "GraphNodesExt" class in "graph_nodes_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["GraphNodes"]


@define(str=False, repr=False, init=False)
class GraphNodes(Archetype):
    """
    **Archetype**: A list of nodes in a graph with optional labels, colors, etc.

    ⚠️ **This is an experimental API! It is not fully supported, and is likely to change significantly in future versions.**
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
            node_ids=None,  # type: ignore[arg-type]
            positions=None,  # type: ignore[arg-type]
            colors=None,  # type: ignore[arg-type]
            labels=None,  # type: ignore[arg-type]
            show_labels=None,  # type: ignore[arg-type]
            radii=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> GraphNodes:
        """Produce an empty GraphNodes, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    node_ids: components.GraphNodeBatch = field(
        metadata={"component": "required"},
        converter=components.GraphNodeBatch._required,  # type: ignore[misc]
    )
    # A list of node IDs.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    positions: components.Position2DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Position2DBatch._optional,  # type: ignore[misc]
    )
    # Optional center positions of the nodes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional colors for the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    # Optional text labels for the node.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    show_labels: components.ShowLabelsBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ShowLabelsBatch._optional,  # type: ignore[misc]
    )
    # Optional choice of whether the text labels should be shown by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    # Optional radii for nodes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
