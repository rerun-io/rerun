# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/tensor.fbs".

# You can extend this class by creating a "TensorExt" class in "tensor_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from .tensor_ext import TensorExt

__all__ = ["Tensor"]


@define(str=False, repr=False, init=False)
class Tensor(TensorExt, Archetype):
    """
    **Archetype**: An N-dimensional array of numbers.

    It's not currently possible to use `send_columns` with tensors since construction
    of `rerun.components.TensorDataBatch` does not support more than a single element.
    This will be addressed as part of <https://github.com/rerun-io/rerun/issues/6832>.

    Example
    -------
    ### Simple tensor:
    ```python
    import numpy as np
    import rerun as rr

    tensor = np.random.randint(0, 256, (8, 6, 3, 5), dtype=np.uint8)  # 4-dimensional tensor

    rr.init("rerun_example_tensor", spawn=True)

    # Log the tensor, assigning names to each dimension
    rr.log("tensor", rr.Tensor(tensor, dim_names=("width", "height", "channel", "batch")))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/tensor_simple/baacb07712f7b706e3c80e696f70616c6c20b367/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/tensor_simple/baacb07712f7b706e3c80e696f70616c6c20b367/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/tensor_simple/baacb07712f7b706e3c80e696f70616c6c20b367/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/tensor_simple/baacb07712f7b706e3c80e696f70616c6c20b367/1200w.png">
      <img src="https://static.rerun.io/tensor_simple/baacb07712f7b706e3c80e696f70616c6c20b367/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in tensor_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            data=None,  # type: ignore[arg-type]
            value_range=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Tensor:
        """Produce an empty Tensor, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        data: datatypes.TensorDataLike | None = None,
        value_range: datatypes.Range1DLike | None = None,
    ) -> Tensor:
        """
        Update only some specific fields of a `Tensor`.

        Parameters
        ----------
        clear:
             If true, all unspecified fields will be explicitly cleared.
        data:
            The tensor data
        value_range:
            The expected range of values.

            This is typically the expected range of valid values.
            Everything outside of the range is clamped to the range for the purpose of colormpaping.
            Any colormap applied for display, will map this range.

            If not specified, the range will be automatically estimated from the data.
            Note that the Viewer may try to guess a wider range than the minimum/maximum of values
            in the contents of the tensor.
            E.g. if all values are positive, some bigger than 1.0 and all smaller than 255.0,
            the Viewer will guess that the data likely came from an 8bit image, thus assuming a range of 0-255.

        """

        kwargs = {
            "data": data,
            "value_range": value_range,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return Tensor(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> Tensor:
        """Clear all the fields of a `Tensor`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            data=[],  # type: ignore[arg-type]
            value_range=[],  # type: ignore[arg-type]
        )
        return inst

    data: components.TensorDataBatch = field(
        metadata={"component": "optional"},
        converter=components.TensorDataBatch._optional,  # type: ignore[misc]
    )
    # The tensor data
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    value_range: components.ValueRangeBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ValueRangeBatch._optional,  # type: ignore[misc]
    )
    # The expected range of values.
    #
    # This is typically the expected range of valid values.
    # Everything outside of the range is clamped to the range for the purpose of colormpaping.
    # Any colormap applied for display, will map this range.
    #
    # If not specified, the range will be automatically estimated from the data.
    # Note that the Viewer may try to guess a wider range than the minimum/maximum of values
    # in the contents of the tensor.
    # E.g. if all values are positive, some bigger than 1.0 and all smaller than 255.0,
    # the Viewer will guess that the data likely came from an 8bit image, thus assuming a range of 0-255.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
