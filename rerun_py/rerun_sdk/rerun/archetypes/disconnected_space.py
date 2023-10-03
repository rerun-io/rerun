# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/disconnected_space.fbs".

# You can extend this class by creating a "DisconnectedSpaceExt" class in "disconnected_space_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components
from .._baseclasses import Archetype
from ..error_utils import catch_and_log_exceptions

__all__ = ["DisconnectedSpace"]


@define(str=False, repr=False, init=False)
class DisconnectedSpace(Archetype):
    """
    **Archetype**: Specifies that the entity path at which this is logged is disconnected from its parent.

    This is useful for specifying that a subgraph is independent of the rest of the scene.

    If a transform or pinhole is logged on the same path, this archetype's components
    will be ignored.

    Example
    -------
    ```python
    import rerun as rr

    rr.init("rerun_example_disconnect_space", spawn=True)

    # These two points can be projected into the same space..
    rr.log("world/room1/point", rr.Points3D([[0, 0, 0]]))
    rr.log("world/room2/point", rr.Points3D([[1, 1, 1]]))

    # ..but this one lives in a completely separate space!
    rr.log("world/wormhole", rr.DisconnectedSpace(True))
    rr.log("world/wormhole/point", rr.Points3D([[2, 2, 2]]))
    ```
    """

    def __init__(self: Any, disconnected_space: components.DisconnectedSpaceLike):
        """Create a new instance of the DisconnectedSpace archetype."""

        # You can define your own __init__ function as a member of DisconnectedSpaceExt in disconnected_space_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(disconnected_space=disconnected_space)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            disconnected_space=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> DisconnectedSpace:
        """Produce an empty DisconnectedSpace, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    disconnected_space: components.DisconnectedSpaceBatch = field(
        metadata={"component": "required"},
        converter=components.DisconnectedSpaceBatch._required,  # type: ignore[misc]
    )
    # Docstring intentionally omitted to hide this field from the docs. See the docs for the __init__ method instead.

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
