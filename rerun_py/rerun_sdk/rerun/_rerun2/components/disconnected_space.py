# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/disconnected_space.fbs".


from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import override_disconnected_space___native_to_pa_array_override  # noqa: F401

__all__ = [
    "DisconnectedSpace",
    "DisconnectedSpaceArray",
    "DisconnectedSpaceArrayLike",
    "DisconnectedSpaceLike",
    "DisconnectedSpaceType",
]


@define
class DisconnectedSpace:
    """
    Specifies that the entity path at which this is logged is disconnected from its parent.

    This is useful for specifying that a subgraph is independent of the rest of the scene.

    If a transform or pinhole is logged on the same path, this component will be ignored.
    """

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    is_disconnected: bool = field(converter=bool)


if TYPE_CHECKING:
    DisconnectedSpaceLike = Union[DisconnectedSpace, bool]
else:
    DisconnectedSpaceLike = Any

DisconnectedSpaceArrayLike = Union[DisconnectedSpace, Sequence[DisconnectedSpaceLike], bool, npt.NDArray[np.bool_]]


# --- Arrow support ---


class DisconnectedSpaceType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.bool_(), "rerun.disconnected_space")


class DisconnectedSpaceArray(BaseExtensionArray[DisconnectedSpaceArrayLike]):
    _EXTENSION_NAME = "rerun.disconnected_space"
    _EXTENSION_TYPE = DisconnectedSpaceType

    @staticmethod
    def _native_to_pa_array(data: DisconnectedSpaceArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_disconnected_space__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/components/_overrides/disconnected_space.py


DisconnectedSpaceType._ARRAY_TYPE = DisconnectedSpaceArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(DisconnectedSpaceType())
