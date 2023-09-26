# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/tensor.fbs".

# You can extend this class by creating a "TensorExt" class in "tensor_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import Archetype
from .tensor_ext import TensorExt

__all__ = ["Tensor"]


@define(str=False, repr=False, init=False)
class Tensor(TensorExt, Archetype):
    """
    A generic n-dimensional Tensor.

    Example
    -------
    ```python

    import rerun as rr
    from numpy.random import default_rng

    rng = default_rng(12345)

    # Create a 4-dimensional tensor
    tensor = rng.uniform(0.0, 1.0, (8, 6, 3, 5))

    rr.init("rerun_example_tensors", spawn=True)

    # Log the tensor, assigning names to each dimension
    rr.log("tensor", rr.Tensor(tensor, names=("width", "height", "channel", "batch")))
    ```
    """

    # __init__ can be found in tensor_ext.py

    data: components.TensorDataBatch = field(
        metadata={"component": "required"},
        converter=components.TensorDataBatch,  # type: ignore[misc]
    )
    """
    The tensor data
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__


if hasattr(TensorExt, "deferred_patch_class"):
    TensorExt.deferred_patch_class(Tensor)
