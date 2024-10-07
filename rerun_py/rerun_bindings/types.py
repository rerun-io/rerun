from __future__ import annotations

from typing import TYPE_CHECKING, Sequence, TypeAlias, Union

if TYPE_CHECKING:
    from rerun._baseclasses import ComponentMixin

    from .rerun_bindings import (
        ComponentColumnDescriptor as ComponentColumnDescriptor,
        ComponentColumnSelector as ComponentColumnSelector,
        TimeColumnDescriptor as TimeColumnDescriptor,
        TimeColumnSelector as TimeColumnSelector,
    )

ComponentLike: TypeAlias = Union[str, type["ComponentMixin"]]

AnyColumn: TypeAlias = Union[
    "TimeColumnDescriptor",
    "ComponentColumnDescriptor",
    "TimeColumnSelector",
    "ComponentColumnSelector",
]

AnyComponentColumn: TypeAlias = Union[
    "ComponentColumnDescriptor",
    "ComponentColumnSelector",
]

ViewContentsLike: TypeAlias = Union[
    str,
    dict[str, Union[AnyColumn, Sequence[ComponentLike]]],
]
