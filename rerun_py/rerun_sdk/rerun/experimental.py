"""
Experimental features for Rerun.

These features are not yet stable and may change in future releases without
going through the normal deprecation cycle.
"""
from __future__ import annotations

from rerun.log.experimental.blueprint import add_space_view, new_blueprint, set_auto_space_views, set_panels
from rerun.log.experimental.text import log_text_box

__all__ = [
    "AnnotationContext",
    "Arrows3D",
    "DisconnectedSpace",
    "LineStrips2D",
    "LineStrips3D",
    "Points2D",
    "Points3D",
    "Transform3D",
    "add_space_view",
    "arch",
    "cmp",
    "dt",
    "log",
    "log",
    "log_text_box",
    "new_blueprint",
    "set_auto_space_views",
    "set_panels",
]

# Next-gen API imports
from ._rerun2 import archetypes as arch
from ._rerun2 import components as cmp
from ._rerun2 import datatypes as dt
from ._rerun2.archetypes import (
    AnnotationContext,
    Arrows3D,
    DisconnectedSpace,
    LineStrips2D,
    LineStrips3D,
    Points2D,
    Points3D,
    Transform3D,
)
from ._rerun2.log import log
