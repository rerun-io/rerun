# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/query_expression.fbs".

# You can extend this class by creating a "QueryExpressionExt" class in "query_expression_ext.py".

from __future__ import annotations

from ... import datatypes
from ..._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["QueryExpression", "QueryExpressionBatch", "QueryExpressionType"]


class QueryExpression(datatypes.Utf8, ComponentMixin):
    """
    **Component**: An individual `QueryExpression` used to filter a set of `EntityPath`s.

    Each expression is either an inclusion or an exclusion expression.
    Inclusions start with an optional `+` and exclusions must start with a `-`.

    Multiple expressions are combined together as part of `SpaceViewContents`.

    The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
    (`/world/**` matches both `/world` and `/world/car/driver`).
    Other uses of `*` are not (yet) supported.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of QueryExpressionExt in query_expression_ext.py

    # Note: there are no fields here because QueryExpression delegates to datatypes.Utf8
    pass


class QueryExpressionType(datatypes.Utf8Type):
    _TYPE_NAME: str = "rerun.blueprint.components.QueryExpression"


class QueryExpressionBatch(datatypes.Utf8Batch, ComponentBatchMixin):
    _ARROW_TYPE = QueryExpressionType()


# This is patched in late to avoid circular dependencies.
QueryExpression._BATCH_TYPE = QueryExpressionBatch  # type: ignore[assignment]
