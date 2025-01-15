# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/view_contents.fbs".

# You can extend this class by creating a "ViewContentsExt" class in "view_contents_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions

__all__ = ["ViewContents"]


@define(str=False, repr=False, init=False)
class ViewContents(Archetype):
    """
    **Archetype**: The contents of a `View`.

    The contents are found by combining a collection of `QueryExpression`s.

    ```diff
    + /world/**           # add everything…
    - /world/roads/**     # …but remove all roads…
    + /world/roads/main   # …but show main road
    ```

    If there is multiple matching rules, the most specific rule wins.
    If there are multiple rules of the same specificity, the last one wins.
    If no rules match, the path is excluded.

    Specifying a path without a `+` or `-` prefix is equivalent to `+`:
    ```diff
    /world/**           # add everything…
    - /world/roads/**   # …but remove all roads…
    /world/roads/main   # …but show main road
    ```

    The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
    (`/world/**` matches both `/world` and `/world/car/driver`).
    Other uses of `*` are not (yet) supported.

    Internally, `EntityPathFilter` sorts the rule by entity path, with recursive coming before non-recursive.
    This means the last matching rule is also the most specific one. For instance:
    ```diff
    + /world/**
    - /world
    - /world/car/**
    + /world/car/driver
    ```

    The last rule matching `/world/car/driver` is `+ /world/car/driver`, so it is included.
    The last rule matching `/world/car/hood` is `- /world/car/**`, so it is excluded.
    The last rule matching `/world` is `- /world`, so it is excluded.
    The last rule matching `/world/house` is `+ /world/**`, so it is included.
    """

    def __init__(self: Any, query: datatypes.Utf8ArrayLike):
        """
        Create a new instance of the ViewContents archetype.

        Parameters
        ----------
        query:
            The `QueryExpression` that populates the contents for the view.

            They determine which entities are part of the view.

        """

        # You can define your own __init__ function as a member of ViewContentsExt in view_contents_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(query=query)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            query=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> ViewContents:
        """Produce an empty ViewContents, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        query: datatypes.Utf8ArrayLike | None = None,
    ) -> ViewContents:
        """
        Update only some specific fields of a `ViewContents`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        query:
            The `QueryExpression` that populates the contents for the view.

            They determine which entities are part of the view.

        """

        kwargs = {
            "query": query,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return ViewContents(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> ViewContents:
        """Clear all the fields of a `ViewContents`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            query=[],  # type: ignore[arg-type]
        )
        return inst

    query: blueprint_components.QueryExpressionBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=blueprint_components.QueryExpressionBatch._optional,  # type: ignore[misc]
    )
    # The `QueryExpression` that populates the contents for the view.
    #
    # They determine which entities are part of the view.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
