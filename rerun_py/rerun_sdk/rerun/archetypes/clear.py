# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/clear.fbs".

# You can extend this class by creating a "ClearExt" class in "clear_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import Archetype
from .clear_ext import ClearExt

__all__ = ["Clear"]


@define(str=False, repr=False, init=False)
class Clear(ClearExt, Archetype):
    """
    Empties all the components of an entity.

    Examples
    --------
    Flat:
    ```python

    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_clear_simple", spawn=True)

    vectors = [(1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (-1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]
    origins = [(-0.5, 0.5, 0.0), (0.5, 0.5, 0.0), (0.5, -0.5, 0.0), (-0.5, -0.5, 0.0)]
    colors = [(200, 0, 0), (0, 200, 0), (0, 0, 200), (200, 0, 200)]

    # Log a handful of arrows.
    for i, (vector, origin, color) in enumerate(zip(vectors, origins, colors)):
        rr2.log(f"arrows/{i}", rr2.Arrows3D(vectors=vector, origins=origin, colors=color))

    # Now clear them, one by one on each tick.
    for i in range(len(vectors)):
        rr2.log(f"arrows/{i}", rr2.Clear(recursive=False))  # or `rr2.Clear.flat()`
    ```

    Recursive:
    ```python

    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_clear_simple", spawn=True)

    vectors = [(1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (-1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]
    origins = [(-0.5, 0.5, 0.0), (0.5, 0.5, 0.0), (0.5, -0.5, 0.0), (-0.5, -0.5, 0.0)]
    colors = [(200, 0, 0), (0, 200, 0), (0, 0, 200), (200, 0, 200)]

    # Log a handful of arrows.
    for i, (vector, origin, color) in enumerate(zip(vectors, origins, colors)):
        rr2.log(f"arrows/{i}", rr2.Arrows3D(vectors=vector, origins=origin, colors=color))

    # Now clear all of them at once.
    rr2.log("arrows", rr2.Clear(recursive=True))  # or `rr2.Clear.recursive()`
    ```
    """

    # __init__ can be found in clear_ext.py

    settings: components.ClearSettingsBatch = field(
        metadata={"component": "required"},
        converter=components.ClearSettingsBatch,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__


if hasattr(ClearExt, "deferred_patch_class"):
    ClearExt.deferred_patch_class(Clear)
