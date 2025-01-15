# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/clear.fbs".

# You can extend this class by creating a "ClearExt" class in "clear_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from ..error_utils import catch_and_log_exceptions
from .clear_ext import ClearExt

__all__ = ["Clear"]


@define(str=False, repr=False, init=False)
class Clear(ClearExt, Archetype):
    """
    **Archetype**: Empties all the components of an entity.

    The presence of a clear means that a latest-at query of components at a given path(s)
    will not return any components that were logged at those paths before the clear.
    Any logged components after the clear are unaffected by the clear.

    This implies that a range query that includes time points that are before the clear,
    still returns all components at the given path(s).
    Meaning that in practice clears are ineffective when making use of visible time ranges.
    Scalar plots are an exception: they track clears and use them to represent holes in the
    data (i.e. discontinuous lines).

    Example
    -------
    ### Flat:
    ```python
    import rerun as rr

    rr.init("rerun_example_clear", spawn=True)

    vectors = [(1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (-1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]
    origins = [(-0.5, 0.5, 0.0), (0.5, 0.5, 0.0), (0.5, -0.5, 0.0), (-0.5, -0.5, 0.0)]
    colors = [(200, 0, 0), (0, 200, 0), (0, 0, 200), (200, 0, 200)]

    # Log a handful of arrows.
    for i, (vector, origin, color) in enumerate(zip(vectors, origins, colors)):
        rr.log(f"arrows/{i}", rr.Arrows3D(vectors=vector, origins=origin, colors=color))

    # Now clear them, one by one on each tick.
    for i in range(len(vectors)):
        rr.log(f"arrows/{i}", rr.Clear(recursive=False))  # or `rr.Clear.flat()`
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/1200w.png">
      <img src="https://static.rerun.io/clear_simple/2f5df95fcc53e9f0552f65670aef7f94830c5c1a/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in clear_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            is_recursive=None,
        )

    @classmethod
    def _clear(cls) -> Clear:
        """Produce an empty Clear, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        is_recursive: datatypes.BoolLike | None = None,
    ) -> Clear:
        """Update only some specific fields of a `Clear`."""

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "is_recursive": is_recursive,
            }

            if clear:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def clear_fields(cls) -> Clear:
        """Clear all the fields of a `Clear`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            is_recursive=[],
        )
        return inst

    is_recursive: components.ClearIsRecursiveBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ClearIsRecursiveBatch._converter,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
