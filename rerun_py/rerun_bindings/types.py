from __future__ import annotations

from typing import TYPE_CHECKING, TypeAlias, Union

if TYPE_CHECKING:
    from rerun._baseclasses import ComponentMixin

    from .rerun_bindings import (
        ComponentColumnDescriptor as ComponentColumnDescriptor,
        ComponentColumnSelector as ComponentColumnSelector,
        ControlColumnDescriptor as ControlColumnDescriptor,
        ControlColumnSelector as ControlColumnSelector,
        TimeColumnDescriptor as TimeColumnDescriptor,
        TimeColumnSelector as TimeColumnSelector,
    )


ComponentLike: TypeAlias = Union[str, type["ComponentMixin"]]

AnyColumn: TypeAlias = Union[
    "ControlColumnDescriptor",
    "TimeColumnDescriptor",
    "ComponentColumnDescriptor",
    "ControlColumnSelector",
    "TimeColumnSelector",
    "ComponentColumnSelector",
]

AnyComponentColumn: TypeAlias = Union[
    "ComponentColumnDescriptor",
    "ComponentColumnSelector",
]
