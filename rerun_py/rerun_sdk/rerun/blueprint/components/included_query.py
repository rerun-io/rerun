# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/included_query.fbs".

# You can extend this class by creating a "IncludedQueryExt" class in "included_query_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import ComponentBatchMixin

__all__ = ["IncludedQuery", "IncludedQueryBatch", "IncludedQueryType"]


class IncludedQuery(datatypes.Uuid):
    """
    **Component**: Each query id refers to a `QueryExpression` component.

    Unstable. Used for the ongoing blueprint experimentations.
    """

    # You can define your own __init__ function as a member of IncludedQueryExt in included_query_ext.py

    # Note: there are no fields here because IncludedQuery delegates to datatypes.Uuid
    pass


class IncludedQueryType(datatypes.UuidType):
    _TYPE_NAME: str = "rerun.blueprint.components.IncludedQuery"


class IncludedQueryBatch(datatypes.UuidBatch, ComponentBatchMixin):
    _ARROW_TYPE = IncludedQueryType()
