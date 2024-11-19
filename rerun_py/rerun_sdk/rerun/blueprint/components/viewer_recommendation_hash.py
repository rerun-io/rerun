# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/viewer_recommendation_hash.fbs".

# You can extend this class by creating a "ViewerRecommendationHashExt" class in "viewer_recommendation_hash_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["ViewerRecommendationHash", "ViewerRecommendationHashBatch"]


class ViewerRecommendationHash(datatypes.UInt64, ComponentMixin):
    """
    **Component**: Hash of a viewer recommendation.

    The formation of this hash is considered an internal implementation detail of the viewer.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of ViewerRecommendationHashExt in viewer_recommendation_hash_ext.py

    # Note: there are no fields here because ViewerRecommendationHash delegates to datatypes.UInt64
    pass


class ViewerRecommendationHashBatch(datatypes.UInt64Batch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.blueprint.components.ViewerRecommendationHash"


# This is patched in late to avoid circular dependencies.
ViewerRecommendationHash._BATCH_TYPE = ViewerRecommendationHashBatch  # type: ignore[assignment]
