"""
Experimental features for Rerun.

These features are not yet stable and may change in future releases without
going through the normal deprecation cycle.
"""
from __future__ import annotations

from rerun.log.experimental.blueprint import add_space_view, new_blueprint, set_auto_space_views, set_panels

__all__ = [
    "AnnotationContext",
    "ArchetypeLike",
    "Arrows3D",
    "Boxes2D",
    "Boxes3D",
    "Clear",
    "ComponentBatchLike",
    "DepthImage",
    "DisconnectedSpace",
    "Image",
    "IndicatorComponentBatch",
    "LineStrips2D",
    "LineStrips3D",
    "Points2D",
    "Points3D",
    "SegmentationImage",
    "Tensor",
    "TextDocument",
    "TextLog",
    "Transform3D",
    "add_space_view",
    "arch",
    "cmp",
    "dt",
    "log",
    "log_components",
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
    Boxes2D,
    Boxes3D,
    Clear,
    DepthImage,
    DisconnectedSpace,
    Image,
    LineStrips2D,
    LineStrips3D,
    Points2D,
    Points3D,
    SegmentationImage,
    Tensor,
    TextDocument,
    TextLog,
    Transform3D,
)
from ._rerun2.log import ArchetypeLike, ComponentBatchLike, IndicatorComponentBatch, log, log_components
