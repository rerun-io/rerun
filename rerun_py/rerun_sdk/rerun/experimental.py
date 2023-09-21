"""
Experimental features for Rerun.

These features are not yet stable and may change in future releases without
going through the normal deprecation cycle.
"""
from __future__ import annotations

from rerun.log_deprecated.experimental.blueprint import add_space_view, new_blueprint, set_auto_space_views, set_panels

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
    "Mesh3D",
    "Points2D",
    "Points3D",
    "Pinhole",
    "SegmentationImage",
    "Tensor",
    "TextDocument",
    "TextLog",
    "Transform3D",
    "ViewCoordinates",
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
from . import archetypes as arch
from . import components as cmp
from . import datatypes as dt
from .archetypes import (
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
    Mesh3D,
    Pinhole,
    Points2D,
    Points3D,
    SegmentationImage,
    Tensor,
    TextDocument,
    TextLog,
    Transform3D,
    ViewCoordinates,
)
from .log import ArchetypeLike, ComponentBatchLike, IndicatorComponentBatch, log, log_components
