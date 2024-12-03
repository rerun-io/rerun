# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/force_many_body.fbs".

# You can extend this class by creating a "ForceManyBodyExt" class in "force_many_body_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import numpy as np
import pyarrow as pa
from attrs import define, field

from ..._baseclasses import (
    BaseBatch,
)

__all__ = ["ForceManyBody", "ForceManyBodyArrayLike", "ForceManyBodyBatch", "ForceManyBodyLike"]


@define(init=False)
class ForceManyBody:
    """
    **Datatype**: Defines a force that is similar to an electric charge between nodes.

    Positive strengths will push nodes apart, while negative strengths will pull nodes together.
    """

    def __init__(self: Any, enabled: bool, strength: float):
        """
        Create a new instance of the ForceManyBody datatype.

        Parameters
        ----------
        enabled:
            Whether the force is enabled.
        strength:
            The strength of the force.

        """

        # You can define your own __init__ function as a member of ForceManyBodyExt in force_many_body_ext.py
        self.__attrs_init__(enabled=enabled, strength=strength)

    enabled: bool = field(converter=bool)
    # Whether the force is enabled.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    strength: float = field(converter=float)
    # The strength of the force.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


ForceManyBodyLike = ForceManyBody
ForceManyBodyArrayLike = Union[
    ForceManyBody,
    Sequence[ForceManyBodyLike],
]


class ForceManyBodyBatch(BaseBatch[ForceManyBodyArrayLike]):
    _ARROW_DATATYPE = pa.struct([
        pa.field("enabled", pa.bool_(), nullable=False, metadata={}),
        pa.field("strength", pa.float64(), nullable=False, metadata={}),
    ])

    @staticmethod
    def _native_to_pa_array(data: ForceManyBodyArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, ForceManyBody):
            data = [data]

        return pa.StructArray.from_arrays(
            [
                pa.array(np.asarray([x.enabled for x in data], dtype=np.bool_)),
                pa.array(np.asarray([x.strength for x in data], dtype=np.float64)),
            ],
            fields=list(data_type),
        )
