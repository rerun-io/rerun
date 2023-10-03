# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/keypoint_id.fbs".

# You can extend this class by creating a "KeypointIdExt" class in "keypoint_id_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["KeypointId", "KeypointIdBatch", "KeypointIdType"]


class KeypointId(datatypes.KeypointId):
    """
    **Component**: A 16-bit ID representing a type of semantic keypoint within a class.

    `KeypointId`s are only meaningful within the context of a [`rerun.datatypes.ClassDescription`].

    Used to look up an [`rerun.datatypes.AnnotationInfo`] for a Keypoint within the
    [`rerun.components.AnnotationContext`].
    """

    # You can define your own __init__ function as a member of KeypointIdExt in keypoint_id_ext.py

    # Note: there are no fields here because KeypointId delegates to datatypes.KeypointId
    pass


class KeypointIdType(datatypes.KeypointIdType):
    _TYPE_NAME: str = "rerun.components.KeypointId"


class KeypointIdBatch(datatypes.KeypointIdBatch, ComponentBatchMixin):
    _ARROW_TYPE = KeypointIdType()
