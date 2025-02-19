# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/visualizer_overrides.fbs".

# You can extend this class by creating a "VisualizerOverridesExt" class in "visualizer_overrides_ext.py".

from __future__ import annotations

from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)
from ...blueprint import datatypes as blueprint_datatypes
from .visualizer_overrides_ext import VisualizerOverridesExt

__all__ = ["VisualizerOverrides", "VisualizerOverridesBatch"]


class VisualizerOverrides(VisualizerOverridesExt, blueprint_datatypes.Utf8List, ComponentMixin):
    """
    **Component**: Override the visualizers for an entity.

    This component is a stop-gap mechanism based on the current implementation details
    of the visualizer system. It is not intended to be a long-term solution, but provides
    enough utility to be useful in the short term.

    The long-term solution is likely to be based off: <https://github.com/rerun-io/rerun/issues/6626>

    This can only be used as part of blueprints. It will have no effect if used
    in a regular entity.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of VisualizerOverridesExt in visualizer_overrides_ext.py

    # Note: there are no fields here because VisualizerOverrides delegates to datatypes.Utf8List


class VisualizerOverridesBatch(blueprint_datatypes.Utf8ListBatch, ComponentBatchMixin):
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.blueprint.components.VisualizerOverrides")


# This is patched in late to avoid circular dependencies.
VisualizerOverrides._BATCH_TYPE = VisualizerOverridesBatch  # type: ignore[assignment]
